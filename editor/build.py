"""Builds the offline video editor (OpenCut classic, MIT) into app/src-tauri/editor-dist.

    python editor/build.py

Needs git, bun and node. It clones OpenCut at the pinned commit into a temp folder, applies editor/opencut.patch
(static export, no login/database/analytics, project id from the URL) and copies the static site.
"""
import pathlib, shutil, subprocess, sys, tempfile

REPO = "https://github.com/opencut-app/opencut-classic.git"
COMMIT = "cf5e79e919144200294fb9fed22a222592a0aeea"
HERE = pathlib.Path(__file__).resolve().parent
DEST = HERE.parent / "app" / "src-tauri" / "editor-dist"


def run(cmd, cwd):
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=cwd, check=True, shell=(sys.platform == "win32"))


def main():
    with tempfile.TemporaryDirectory(prefix="opencut-") as tmp:
        src = pathlib.Path(tmp) / "src"
        run(["git", "clone", "--quiet", REPO, str(src)], tmp)
        run(["git", "checkout", "--quiet", COMMIT], src)
        run(["git", "apply", "--whitespace=nowarn", str(HERE / "opencut.patch")], src)
        run(["bun", "install", "--frozen-lockfile"], src)
        web = src / "apps" / "web"
        run(["bun", "run", "build"], web)
        if DEST.exists():
            shutil.rmtree(DEST)
        shutil.copytree(web / "out", DEST)
        shutil.copy(HERE / "OPENCUT-LICENSE.txt", DEST / "LICENSE.txt")
    print("Editor built into", DEST)


if __name__ == "__main__":
    main()
