#!/usr/bin/env python3
"""Collect genuine grants from Cargo.lock-resolved crate archives, not SPDX guesses."""
from pathlib import Path
import subprocess,json,hashlib,urllib.request
B=Path(__file__).resolve().parents[1];L=B/'licenses';L.mkdir(exist_ok=True)
meta=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],cwd=B));rows=[]
def save(folder,relative,data):
 p=L/folder/relative;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);return dict(path=str(p.relative_to(B)),bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
for p in sorted(meta['packages'],key=lambda p:(p['name'],p['version'])):
 if p['name']=='country-lookup-native':continue
 root=Path(p['manifest_path']).parent;folder=p['name']+'-'+p['version'];files=[]
 for f in sorted(root.rglob('*')):
  if f.is_file() and any(x in f.name.upper() for x in ['LICENSE','LICENCE','NOTICE','COPYING','COPYRIGHT']):files.append(save(folder,str(f.relative_to(root)),f.read_bytes()))
 row=dict(name=p['name'],version=p['version'],source=p['source'],license=p['license'],files=files)
 if p['name']=='r-efi':
  sha=json.loads((root/'.cargo_vcs_info.json').read_text())['git']['sha1'];row['licenseSourceCommit']=sha
  row['files'].append(save(folder,'AUTHORS',(root/'AUTHORS').read_bytes()))
 if not row['files']:raise RuntimeError('Missing actual license for '+folder)
 rows.append(row)
# Own license is literally backslash-n separated in V1. Preserve original bytes separately;
# the package grant reproduces its exact words with readable line separators.
(B/'LICENSE').write_text((B.parent/'LICENSE').read_text().replace('\\n','\n'))
save('own','LICENSE-original-bytes',(B.parent/'LICENSE').read_bytes());save('raycast-country-lookup','LICENSE',(B/'provenance/upstream-LICENSE').read_bytes());save('countries-5.2.1','LICENSE',(B/'provenance/countries-LICENSE').read_bytes());save('host-review-source','LICENSE',(B/'provenance/host/LICENSE').read_bytes())
info=json.loads(urllib.request.urlopen('https://registry.npmjs.org/@raycast%2futils/2.2.7',timeout=30).read())
(B/'provenance/utils-license-source.json').write_text(json.dumps(dict(gitHead=info['gitHead'],version=info['version'],license=info['license'],author=info['author'],dist=info['dist'],note='Publisher declares MIT. NPM archive contains no LICENSE; exact gitHead is not publicly available in raycast/utils. No license file has been invented. QA-only source excerpts retain this declaration and attribution; complete genuine Raycast extensions MIT grant is separately included.'),indent=2)+'\n')
save('raycast-utils-2.2.7','DECLARATION.json',(B/'provenance/utils-license-source.json').read_bytes())
(B/'reports/dependency-grants.json').write_text(json.dumps(dict(note='All Cargo.lock-resolved packages, including target-specific/dev dependencies: a superset of one executable linked graph. Original complete grant/notice files copied byte-for-byte.',packages=rows),indent=2)+'\n')
print('Genuine grants collected for',len(rows),'locked Rust packages plus own/upstream/SDK/host-review/JS-source notices.')
