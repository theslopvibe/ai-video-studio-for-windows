# Video editor: credits

The built-in **Video editor** page is [OpenCut](https://github.com/opencut-app/opencut-classic) ("OpenCut classic"),
a free and open source video editor by the OpenCut contributors, released under the **MIT License**
(see `OPENCUT-LICENSE.txt`). It is not our work: we vendor it unmodified in behaviour and only change how it is built.

What `opencut.patch` changes (and nothing else):

- static export, so it runs offline from a local server inside AI Video Studio;
- removes the website parts that need servers (login, database, rate limiting, blog, analytics script, bot check);
- reads the project id from the address (`?id=`) instead of a server route;
- adds the missing `isShortcutKey` helper that the pinned upstream commit imports but does not define.

Build it with `python editor/build.py` (pinned commit is in that file). Newer upstream versions are welcome:
change the commit, re-create the patch, and test.
