"""RunPod Serverless (queue) handler that runs the SAME Voice Studio backend as the desktop app (image
ghcr.io/debpalash/voicestudio:<version>) and forwards text-to-speech jobs to its /generate route, so the
voice comes out the same as on the owner's PC (same model, same seed, same speed, same instruct).

Models, voices and settings live on the network volume (/runpod-volume/voicestudio): nothing is downloaded twice.
Job input:  {"text", "language", "speed", "seed", "instruct", "ref_text", "ref_audio_b64", "ref_ext"}
Job output: {"audio_b64" (wav), "seconds"}  or {"error": "..."}
The licence of the models (k2-fsa/OmniVoice CC-BY-NC etc.) was accepted by the owner of the account; the worker
records that acceptance with the exact terms the backend reports (fingerprints), the same call the app's UI makes.
"""
import base64, json, os, secrets, subprocess, sys, time, urllib.request, urllib.error, uuid, wave, io

DATA = os.environ.get("VS_DATA_DIR", "/runpod-volume/voicestudio")
PORT = 3900
KEY = secrets.token_urlsafe(24)
BASE = f"http://127.0.0.1:{PORT}"
ACCEPT_LICENCES = os.environ.get("VS_ACCEPT_LICENCES") == "1"

os.makedirs(DATA, exist_ok=True)
env = dict(os.environ, OMNIVOICE_API_KEY=KEY, OMNIVOICE_DATA_DIR=DATA, HF_HOME=os.path.join(DATA, "huggingface"),
           OMNIVOICE_ANALYTICS_DISABLED="1")
proc = subprocess.Popen(["python3", "backend/docker_entrypoint.py"], cwd="/app", env=env)


def call(method, path, body=None, headers=None, timeout=60):
    h = {"Authorization": f"Bearer {KEY}"}
    h.update(headers or {})
    req = urllib.request.Request(BASE + path, data=body, headers=h, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def wait_up():
    for _ in range(300):
        if proc.poll() is not None:
            raise RuntimeError(f"Voice Studio exited with code {proc.returncode}")
        try:
            if call("GET", "/profiles", timeout=5)[0] == 200:
                return
        except Exception:
            pass
        time.sleep(2)
    raise RuntimeError("Voice Studio did not start within 10 minutes")


def multipart(fields, files):
    b = "aivs" + uuid.uuid4().hex
    out = b""
    for k, v in fields.items():
        out += f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode()
    for k, (name, data) in files.items():
        out += f'--{b}\r\nContent-Disposition: form-data; name="{k}"; filename="{name}"\r\nContent-Type: application/octet-stream\r\n\r\n'.encode() + data + b"\r\n"
    out += f"--{b}--\r\n".encode()
    return out, {"Content-Type": f"multipart/form-data; boundary={b}"}


def accept_licences(detail):
    for m in detail.get("models", []):
        body = json.dumps({"repo_id": m["repo_id"], "fingerprint": m["fingerprint"], "accepted": True}).encode()
        s, r = call("POST", "/models/licenses/accept", body, {"Content-Type": "application/json"})
        print("licence", m["repo_id"], s, flush=True)


def volume_ops(i):
    """Maintenance jobs on the network volume (no GPU work): {"op":"ls"} shows what is on it, {"op":"download","files":[{"dir","url"}]}
    fetches model files into /runpod-volume/ComfyUI/models/<dir> (skips complete files, resumes .part files)."""
    import shutil, threading
    base = "/runpod-volume/ComfyUI/models"
    if i["op"] == "ls":
        tot, used, free = shutil.disk_usage("/runpod-volume")
        files = []
        for root, _, names in os.walk("/runpod-volume"):
            for n in names:
                p = os.path.join(root, n)
                try:
                    files.append((os.path.getsize(p), p))
                except OSError:
                    pass
        files.sort(reverse=True)
        return {"disk_gb": [round(tot / 1e9, 1), round(used / 1e9, 1), round(free / 1e9, 1)], "files": [[p, round(sz / 1e9, 2)] for sz, p in files[:200]]}
    res, lock = {}, threading.Lock()

    def get(d, url):
        name = url.rsplit("/", 1)[1]
        os.makedirs(f"{base}/{d}", exist_ok=True)
        final, part = f"{base}/{d}/{name}", f"{base}/{d}/{name}.part"
        try:
            if os.path.exists(final) and os.path.getsize(final) > 1_000_000:
                out = ["have", os.path.getsize(final)]
            else:
                done = os.path.getsize(part) if os.path.exists(part) else 0
                req = urllib.request.Request(url, headers={"Range": f"bytes={done}-"} if done else {})
                with urllib.request.urlopen(req, timeout=120) as r, open(part, "ab" if done else "wb") as f:
                    total = done + int(r.headers.get("Content-Length", 0))
                    while True:
                        b = r.read(8 << 20)
                        if not b:
                            break
                        f.write(b)
                if total and os.path.getsize(part) != total:
                    raise RuntimeError(f"size {os.path.getsize(part)} != {total}")
                os.replace(part, final)
                out = ["done", os.path.getsize(final)]
        except Exception as e:
            out = ["FAILED", str(e)]
        with lock:
            res[f"{d}/{name}"] = out

    ts = [threading.Thread(target=get, args=(f["dir"], f["url"])) for f in i.get("files", []) if str(f.get("url", "")).startswith("https://huggingface.co/")]
    [t.start() for t in ts]
    [t.join() for t in ts]
    return {"files": res}


def handler(job):
    i = job["input"]
    if i.get("op") in ("ls", "download"):
        return volume_ops(i)
    text = str(i.get("text", "")).strip()
    ref = i.get("ref_audio_b64")
    if not text or not ref:
        return {"error": "text and ref_audio_b64 are required"}
    ext = "".join(c for c in str(i.get("ref_ext", "wav")) if c.isalnum())[:5] or "wav"
    fields = {"text": text, "language": str(i.get("language", "English")), "speed": str(i.get("speed", 1.0)), "engine": "omnivoice"}
    if i.get("seed") is not None:
        fields["seed"] = str(int(i["seed"]))
    if i.get("instruct"):
        fields["instruct"] = str(i["instruct"])
    if i.get("ref_text"):  # exact transcript of the sample: no speech-to-text model is needed on the worker
        fields["ref_text"] = str(i["ref_text"])
    body, hd = multipart(fields, {"ref_audio": (f"ref.{ext}", base64.b64decode(ref))})
    for attempt in range(2):
        s, r = call("POST", "/generate", body, hd, timeout=1500)
        if s == 403 and ACCEPT_LICENCES and attempt == 0:
            try:
                accept_licences(json.loads(r)["detail"])
                continue
            except Exception as e:
                return {"error": f"licence step failed: {e}"}
        break
    if s != 200 or len(r) < 1000:
        return {"error": f"Voice Studio {s}: {r[:300].decode('utf8', 'replace')}"}
    try:
        with wave.open(io.BytesIO(r)) as w:
            secs = w.getnframes() / w.getframerate()
    except Exception:
        secs = 0
    return {"audio_b64": base64.b64encode(r).decode(), "seconds": secs}


if __name__ == "__main__":
    wait_up()
    print("Voice Studio is up", flush=True)
    import runpod
    runpod.serverless.start({"handler": handler})
