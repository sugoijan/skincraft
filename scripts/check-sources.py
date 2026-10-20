#!/usr/bin/env python3
"""Assert editor/game dependencies resolve to Git, or to local patches."""
import argparse,json,subprocess
p=argparse.ArgumentParser()
p.add_argument('--local',action='store_true')
p.add_argument('--manifest-path',default='Cargo.toml')
a=p.parse_args()
args=['cargo']
if a.local: args+=['--config','.cargo/config.local.toml']
args+=['metadata','--format-version','1','--manifest-path',a.manifest_path]
if not a.local: args+=['--locked']
m=json.loads(subprocess.check_output(args))
packages=[v for v in m['packages'] if v['name'].startswith(('ddi-','photocraft-','vectorcraft-')) and v['id']!=m['resolve']['root']]
assert packages,'No editor/game dependencies resolved'
for v in packages:
 source=v.get('source')
 if a.local: assert source is None,(v['name'],source)
 else: assert source and source.startswith('git+https://github.com/sugoijan/'),(v['name'],source)
print(f'Verified {len(packages)} editor/game crates from '+('local paths' if a.local else 'pinned Git sources'))
