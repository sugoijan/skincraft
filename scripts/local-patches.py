#!/usr/bin/env python3
"""Generate ignored local Cargo patches for all crates in sibling workspaces."""
import argparse, pathlib, tomllib
p=argparse.ArgumentParser()
base=pathlib.Path(__file__).resolve().parents[1]
for name,default in [('photocraft','photocraft'),('vectorcraft','vectorcraft'),('ddi','DitoDitoInfinito')]:
 p.add_argument('--'+name,type=pathlib.Path,default=base.parent/default)
a=p.parse_args()
parts=['# Generated machine-local overrides; never commit this file.\n']
for name,repo in [('photocraft','photocraft'),('vectorcraft','vectorcraft'),('ddi','ditoditoinfinito')]:
 folder=getattr(a,name).resolve()
 if not (folder/'Cargo.toml').is_file(): p.error(f'Missing checkout: {folder}')
 parts.append(f'[patch."https://github.com/sugoijan/{repo}.git"]')
 # Include all local packages, even currently unused optional crates.
 for manifest in sorted(folder.rglob('Cargo.toml')):
  if any(x in manifest.relative_to(folder).parts for x in ['target','.git','.local','node_modules']): continue
  data=tomllib.loads(manifest.read_text())
  package=data.get('package',{}).get('name')
  if package:
   import json
   parts.append(f'{package} = {{ path = {json.dumps(str(manifest.parent))} }}')
 parts.append('')
out=base/'.cargo/config.local.toml'
out.parent.mkdir(exist_ok=True)
out.write_text('\n'.join(parts))
print(f'Wrote {out}')
