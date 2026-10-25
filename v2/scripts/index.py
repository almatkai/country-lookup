#!/usr/bin/env python3
"""Final unstaged review inventory; no git add/commit or generated archive inclusion."""
from pathlib import Path
import subprocess,json,hashlib
B=Path(__file__).resolve().parents[1];R=B.parent;SOURCE='3db7a9e69eadc4d0ba53895ef686305329828e27'
def git(*args):return subprocess.check_output(['git',*args],cwd=R)
assert Path(git('rev-parse','--show-toplevel').decode().strip()).resolve()==R.resolve()
assert git('branch','--show-current').decode().strip()=='feat/native-v2-rust'
assert git('rev-parse','HEAD').decode().strip()==SOURCE
assert not git('diff','--cached','--name-only'), 'Leave all review work unstaged'
append=b'\n## Native Rust v2\n\nSee [the standalone Rust implementation and review notes](v2/README.md).\n'
sha=lambda b:hashlib.sha256(b).hexdigest()
base=git('ls-tree','-r','--name-only','-z',SOURCE).decode().split('\0');v1=[]
for f in filter(None,base):
 original=git('show',SOURCE+':'+f);actual=(R/f).read_bytes();assert actual==original+(append if f=='README.md' else b''), 'V1 byte drift: '+f
 v1.append(dict(path=f,originalBytes=len(original),originalSha256=sha(original),currentSha256=sha(actual),changedOnlyByAuthorizedReadmeAppend=f=='README.md'))
paths=sorted(set(filter(None,git('ls-files','--cached','--others','--exclude-standard','-z').decode().split('\0'))));items=[]
for f in paths:
 assert f in base or f.startswith('v2/'), 'Out-of-scope new file: '+f
 if f=='v2/INDEX.json':continue
 p=R/f;assert p.is_file() and not p.is_symlink();data=p.read_bytes();items.append(dict(path=f,bytes=len(data),sha256=sha(data)))
# Check declared source fixtures are visible even though generated dist/dependencies are ignored.
fixtures=['v2/qa/oracle.json','v2/qa/adapter-view.json','v2/qa/failure-view.json','v2/qa/source-oracle.mjs','v2/qa/protocol-smoke.py','v2/qa/renderer-schema.mjs','v2/qa/package.json','v2/qa/package-lock.json']
assert all(f in paths for f in fixtures)
assert not any(f.endswith('.zip') or '/node_modules/' in f or f.startswith(('v2/target/','v2/dist/','v2/qa/scratch/','v2/qa/isolated/','v2/qa/v1/')) for f in paths)
iso=json.loads((B/'reports/isolated-build.json').read_text())
for f,digest in iso['sourceInputs'].items():assert sha((B/f).read_bytes())==digest,'Independent build input changed: '+f
package=json.loads((B/'reports/package-inventory.json').read_text());assert package['executableSha256']==iso['binarySha256']
report=dict(format=1,excludesOnlyItself='v2/INDEX.json',scope='all Git-visible tracked and untracked source/review files, without staging',sourceCommit=SOURCE,branch='feat/native-v2-rust',worktrees=git('worktree','list','--porcelain').decode(),v1=v1,sourceFixturesVisible=fixtures,ignoredGeneratedRoots=['v2/target','v2/dist','v2/qa/node_modules','v2/qa/.npm','v2/qa/v1','v2/qa/scratch','v2/qa/isolated','v2/qa/isolated-final','v2/qa/isolated-review','v2/qa/disposable-root-node_modules'],files=items,totalFiles=len(items),totalBytes=sum(f['bytes'] for f in items))
(B/'INDEX.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('INDEX:',len(items),'Git-visible files;',len(v1),'V1 objects byte-verified; final independent build input hashes match; no ZIP/dependency tree staged.')
