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


def handler(job):
    i = job["input"]
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
