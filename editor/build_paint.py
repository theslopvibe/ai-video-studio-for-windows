"""Builds the offline image editor (miniPaint, MIT) with our AI menu into app/src-tauri/paint-dist.

    python editor/build_paint.py

Needs git and node/npm. Clones miniPaint at the pinned commit, adds editor/paint/ai.js and an "AI" menu, builds with webpack.
Dependency install scripts are disabled (--ignore-scripts): webpack and babel need none.
"""
import pathlib, shutil, subprocess, sys, tempfile

REPO = "https://github.com/viliusle/miniPaint.git"
COMMIT = "a79733eb803fc97084ef0ee4faa96b031e69e1c0"
HERE = pathlib.Path(__file__).resolve().parent
DEST = HERE.parent / "app" / "src-tauri" / "paint-dist"

AI_MENU = """	{
		name: 'AI',
		children: [
			{ name: 'Generate image into layer', ellipsis: true, target: 'ai/ai.generate' },
			{ name: 'Change picture with words', ellipsis: true, target: 'ai/ai.edit' },
			{ name: 'Upscale layer (sharper, 4x larger)', target: 'ai/ai.upscale' },
			{ name: 'Remove background', target: 'ai/ai.remove_background' }
		]
	},
	{
		name: 'Help',"""


def run(cmd, cwd):
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=cwd, check=True, shell=(sys.platform == "win32"))


def main():
    with tempfile.TemporaryDirectory(prefix="minipaint-") as tmp:
        src = pathlib.Path(tmp) / "src"
        run(["git", "clone", "--quiet", REPO, str(src)], tmp)
        run(["git", "checkout", "--quiet", COMMIT], src)
        (src / "src/js/modules/ai").mkdir()
        shutil.copy(HERE / "paint" / "ai.js", src / "src/js/modules/ai/ai.js")
        menu = src / "src/js/config-menu.js"
        text = menu.read_text(encoding="utf8")
        old = "\t{\n\t\tname: 'Help',"
        assert text.count(old) == 1, "menu anchor not found"
        menu.write_text(text.replace(old, AI_MENU), encoding="utf8")
        run(["npm", "ci", "--no-audit", "--no-fund", "--ignore-scripts"], src)
        run(["npm", "run", "build"], src)
        if DEST.exists():
            shutil.rmtree(DEST)
        DEST.mkdir(parents=True)
        shutil.copy(src / "index.html", DEST / "index.html")
        shutil.copytree(src / "dist", DEST / "dist", ignore=shutil.ignore_patterns("*.map"))
        shutil.copytree(src / "images", DEST / "images")
        shutil.copy(src / "MIT-LICENSE.txt", DEST / "LICENSE.txt")
    print("Image editor built into", DEST)


if __name__ == "__main__":
    main()
