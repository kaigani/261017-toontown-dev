from pathlib import Path
import re
root = Path(__file__).resolve().parents[2]
text = (root / 'crates/executor/src/commands/keymap.rs').read_text(encoding='utf-8')
block = text.split('pub(crate) struct SetParams {', 1)[1].split('\n}', 1)[0]
assert not re.search(r'#\[schemars\(length\(max\s*=\s*\d+\)\)\]\s*(?:pub\s+)?(?:command_id|keys):', block), 'C41-R1-13: schema maximum pre-empts keymap domain errors'
