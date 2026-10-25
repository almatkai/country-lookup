#!/usr/bin/env python3
"""Generate explicitly isolated seams and review-only patches from immutable host bytes."""
from pathlib import Path
import difflib
B=Path(__file__).resolve().parents[1];R=(B/'provenance/host/src-tauri/src/native_core/extensions/registry.rs').read_text();P=(B/'provenance/host/src-tauri/src/native_core/extensions/process_install.rs').read_text()
def fn(s,needle):
 a=s.index(needle);i=s.index('{',a);n=1;j=i+1
 while n:
  n+=(s[j]=='{')-(s[j]=='}');j+=1
 return s[a:j]
F=R.replace('    data_dir: PathBuf,','    data_dir: PathBuf,\n    // Modified: exact verified active package root; preferences keep path.\n    resource_root: PathBuf,',1)
F=F.replace('            let mut manifest = if native {','            let mut resource_root = path.clone();\n            let mut manifest = if native {',1).replace('                    Ok(package) => package.manifest,','                    Ok(package) => { resource_root = package.dir; package.manifest },',1)
F=F.replace('                path,\n                manifest,','                path,\n                resource_root,\n                manifest,',1).replace('        path,\n        manifest: Value::Null,','        resource_root: path.clone(),\n        path,\n        manifest: Value::Null,',1)
for name in ['fn icon_path','fn command_icon_path']:
 old=fn(F,name);new=old.replace('extension.path.join(relative)','extension.resource_root.join(relative)').replace('extension.path.join("assets")','extension.resource_root.join("assets")');F=F.replace(old,new)
a=F.index('    let native = if id.starts_with("raycast.")');b=F.index('    if let Some(extension) = native',a)
F=F[:a]+'''    // Modified: only exact aliases from a verified installed manifest can route natively.
    let native = if id.starts_with("com.tezbar.native.") {
        installed(ctx, id).ok()
    } else {
        collect(ctx).into_iter().find(|entry| {
            entry.id.starts_with("com.tezbar.native.")
                && entry.manifest["replaces"].as_array().is_some_and(|aliases|
                    aliases.iter().any(|alias| alias.as_str() == Some(id)))
        })
    };
'''+F[b:]
(B/'integration/registry-active-resources-and-exact-alias.patch').write_text(''.join(difflib.unified_diff(R.splitlines(True),F.splitlines(True),fromfile='a/src-tauri/src/native_core/extensions/registry.rs',tofile='b/src-tauri/src/native_core/extensions/registry.rs')))
D=(B/'provenance/host/src/renderer/src/raycast-api/detail-runtime.tsx').read_text();DF=D.replace('onClick={onRunPrimaryAction}','onClick={() => onRunPrimaryAction()}');(B/'integration/detail-primary-click.patch').write_text(''.join(difflib.unified_diff(D.splitlines(True),DF.splitlines(True),fromfile='a/src/renderer/src/raycast-api/detail-runtime.tsx',tofile='b/src/renderer/src/raycast-api/detail-runtime.tsx')))
pre='use serde_json::Value; use std::path::{Path,PathBuf,Component};\n#[derive(Clone)]\n'
modules=[]
for name,src in [('before',R),('after',F)]:
 struct=fn(src,'pub(super) struct Installed')
 body='\n'.join(fn(src,n) for n in ['fn slug','pub(super) fn installed_command','fn platform','fn compatible','fn fallback','fn icon_path','fn command_icon_path'])
 stub='''
#[derive(Clone)] pub struct CoreContext { pub entries:Vec<Installed> }
fn collect(ctx:&CoreContext)->Vec<Installed>{ctx.entries.clone()}
fn installed(ctx:&CoreContext,id:&str)->Result<Installed,String>{let wanted=slug(id)?;let canonical=id.starts_with("com.tezbar.native.");ctx.entries.iter().find(|e|e.slug==wanted&&e.id.starts_with("com.tezbar.native.")==canonical).cloned().ok_or("missing".into())}
'''
 methods='''
pub fn probe(root:&Path,resource:&Path)->(bool,bool,bool,bool) {
 let entry=Installed{id:"com.tezbar.native.country-lookup".into(),slug:"country-lookup".into(),path:root.into(),data_dir:root.into(),RESOURCE manifest:serde_json::json!({"icon":"resources/extension_iconv1.png","replaces":["com.tezbar.country-lookup","raycast.country-lookup"],"commands":[{"name":"index","runtime":"rust","platforms":["macOS","Windows","Linux"]}]})};
 let icon=icon_path(&entry).is_some();let ctx=CoreContext{entries:vec![entry]};
 (icon,installed_command(&ctx,"com.tezbar.country-lookup","index").is_some(),installed_command(&ctx,"raycast.country-lookup","index").is_some(),installed_command(&ctx,"com.tezbar.other","index").is_none())
}
'''.replace('RESOURCE', 'resource_root:resource.into(),' if name=='after' else '')
 modules.append('mod '+name+' {\n'+pre+struct+'\n'+body+stub+methods+'\n}')
# Exact actual runtime validator; launcher descriptors absent, explicit unused seam.
cap=P[P.index('const CAPABILITY_GROUPS'):P.index('// Shared by installer')].replace('pub(super) const METHODS','const METHODS')
validator='\n'.join(fn(P,n) for n in ['pub(super) fn component','pub(super) fn relative','pub(super) fn validate'])
body='''#![allow(dead_code, unused_variables, clippy::redundant_closure, clippy::nonminimal_bool)]
// Generated from host ea100cee; only dependency seams below are synthetic.
mod v2_runtime {pub fn canonical_id(slug:&str)->String{format!("com.tezbar.native.{slug}")}}
'''+ '\n'.join(modules)+'''
mod launcher {pub fn validate(v:&serde_json::Value)->Result<(),String>{if v.get("launcherContributions").is_some() || v["commands"].as_array().unwrap().iter().any(|c|c.get("launcher").is_some()){return Err("Launcher is outside this test seam".into())}Ok(())}}
mod runtime {
use serde_json::Value;use std::{path::PathBuf,collections::HashSet};
'''+cap+validator+'''
}
#[test] fn actual_registry_before_after_resource_root_and_aliases(){
 let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");std::fs::create_dir_all(&root).unwrap();let temp=tempfile::tempdir_in(root).unwrap();let package=temp.path().join("versions/v1");std::fs::create_dir_all(package.join("resources")).unwrap();std::fs::write(package.join("resources/extension_iconv1.png"),b"synthetic icon path probe").unwrap();
 assert_eq!(before::probe(temp.path(),&package),(false,false,true,true));assert_eq!(after::probe(temp.path(),&package),(true,true,true,true));
}
#[test] fn actual_runtime_manifest_validation(){
 let manifest:serde_json::Value=serde_json::from_str(include_str!("../extension.json")).unwrap();assert!(runtime::validate(&manifest).is_ok());
 for (k,v) in [("schemaVersion",serde_json::json!(2)),("id",serde_json::json!("com.tezbar.other")),("capabilities",serde_json::json!(["core.any"])),("execution",serde_json::json!("in-process"))]{let mut bad=manifest.clone();bad[k]=v;assert!(runtime::validate(&bad).is_err())}
 for (k,v) in [("refreshMs",serde_json::json!(1)),("timeoutMs",serde_json::json!(300001)),("concurrency",serde_json::json!("anything"))]{let mut bad=manifest.clone();bad["commands"][0][k]=v;assert!(runtime::validate(&bad).is_err())}
}
'''
(B/'tests/host_seams.rs').write_text(body)
