#!/usr/bin/env python3
"""Disposable exact-lock V1 QA, entirely inside v2/qa. Never invokes a Raycast build."""
from pathlib import Path
import subprocess
B=Path(__file__).resolve().parents[1];R=B.parent;D=B/'qa/v1';D.mkdir(exist_ok=True)
assert subprocess.check_output(['git','rev-parse','--show-toplevel'],cwd=R).decode().strip()==str(R)
ref='3db7a9e69eadc4d0ba53895ef686305329828e27'
for f in subprocess.check_output(['git','ls-tree','-r','--name-only',ref],cwd=R).decode().splitlines():
 p=D/f;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(subprocess.check_output(['git','show',ref+':'+f],cwd=R))
assert (D/'package.json').is_file()
subprocess.run(['npm','ci','--ignore-scripts','--no-audit','--no-fund','--cache',str(B/'qa/.npm')],cwd=D,check=True)
subprocess.run(['npm','test'],cwd=D,check=True);subprocess.run(['npm','run','typecheck'],cwd=D,check=True)
subprocess.run(['npm','ci','--ignore-scripts','--no-audit','--no-fund','--cache',str(B/'qa/.npm')],cwd=B/'qa',check=True)
