#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,shutil
B=Path(__file__).resolve().parents[1];R=B.parent
OWN='3db7a9e69eadc4d0ba53895ef686305329828e27';SDK='e955911f0bf5b558f9d3eb5183df0adc95c22af2';HOST='ea100cee3f69711de12485ba061950c5fc65258f';UP='6734d85992290066fd31a7f5bee3cca3d3545a1a'
host=R.parent/'Tezbar' # Explicit known external baseline; no working-file reads.
host=Path('/Users/almat/Desktop/code/Tezbar APP/Tezbar');sdk=R.parent/'tezbar-extension-sdk-rust'
for f in ['src-tauri/src/native_core/platform.rs','src-tauri/src/native_core/launcher/clipboard.rs','src/shared/extensionRuntime.ts']:
 data=subprocess.check_output(['git','-C',str(host),'show',HOST+':'+f]);p=B/'provenance/host'/f;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
sha=lambda b:hashlib.sha256(b).hexdigest()
blob=lambda b:hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()
def object_rows(repo,ref,paths):
 out=[]
 for f in paths:
  data=subprocess.check_output(['git','-C',str(repo),'show',ref+':'+f]);out.append(dict(path=f,gitBlob=subprocess.check_output(['git','-C',str(repo),'rev-parse',ref+':'+f]).decode().strip(),bytes=len(data),sha256=sha(data)))
 return out
own_paths=subprocess.check_output(['git','ls-tree','-r','--name-only',OWN],cwd=R).decode().splitlines()
host_paths=[str(p.relative_to(B/'provenance/host')) for p in sorted((B/'provenance/host').rglob('*')) if p.is_file()]
sdk_paths=['Cargo.toml','SDK_API.md','HOST_INTEGRATION.md','PROTOCOL.md','LICENSE','NOTICE','src/lib.rs','src/context.rs','src/storage.rs','src/runner.rs','src/stdio.rs']
upstream=[]
for f,local in [('extensions/country-lookup/package.json',B/'provenance/upstream-package.json'),('LICENSE',B/'provenance/upstream-LICENSE'),('extensions/country-lookup/assets/extension_iconv1.png',B/'resources/extension_iconv1.png')]:
 data=local.read_bytes();upstream.append(dict(path=f,rawUrl=f'https://raw.githubusercontent.com/raycast/extensions/{UP}/{f}',computedGitBlob=blob(data),bytes=len(data),sha256=sha(data)))
report=dict(own=dict(url='https://github.com/almatkai/country-lookup',commit=OWN,actualFreshOriginMain=OWN,branch='feat/native-v2-rust',objects=object_rows(R,OWN,own_paths)),sdk=dict(url='https://github.com/almatkai/tezbar-extension-sdk-rust.git',commit=SDK,version='0.1.1',objects=object_rows(sdk,SDK,sdk_paths)),host=dict(commit=HOST,readMode='git show of selected external baseline only; no working-file dependency, checkout or build',objects=object_rows(host,HOST,host_paths)),upstream=dict(commit=UP,objects=upstream),lockedCountries=dict(version='5.2.1',integrity=json.loads((R/'package-lock.json').read_text())['packages']['node_modules/@yusifaliyevpro/countries']['integrity']))
(B/'provenance/ORIGINS.json').write_text(json.dumps(report,indent=2)+'\n')
(B/'provenance/host-objects.json').write_text(json.dumps(report['host']['objects'],indent=2)+'\n')
shutil.copyfile(B/'qa/scratch/xerj-final.txt',B/'reports/xerj-final.txt')
(B/'reports/xerj.md').write_text('''# XERJ coverage

Canonical root: `/Users/almat/Desktop/code/Tezbar APP/rust-extension-ports/country-lookup`.
Prefix: `ax-804a62c6a012`. State: `~/.local/share/xerj/workspaces/ax-804a62c6a012`.
Skill read/disclosed before first search. Local lexical server only, feedback disabled.
Initial autoindex: generation 1, 19 files / 96 records, all 13 code files indexed;
exit 3 completed-with-junk, seven non-code skipped/junk files. Source search used the
canonical prefix before full-file reads and exhaustive rg/Git/object/hash checks.
Final autoindex: 420 discovered files, then exit 1 on frozen dataset/schema mismatch
for v2/Cargo.toml (full log adjacent). No schema mutation, alternate prefix, global
restart, unbounded retry or claimed complete v2 index. v2 coverage is incomplete;
full files, rg, Git objects, executable tests and final hashes are authoritative.
No 429/RSS restart workaround was used.
''')
