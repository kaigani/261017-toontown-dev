"""Every front-end failure fallback must have kernel message help; no build required."""
import pathlib, re
root=pathlib.Path(__file__).resolve().parents[2]
help_text=(root/'crates/executor/src/help.rs').read_text(encoding='utf-8')
tips=(root/'crates/executor/src/commands/ui_shell.rs').read_text(encoding='utf-8').split('fn unknown_message')[0]
known=set(re.findall(r'\("([\w.]+)",',help_text+tips))
source=(root/'desktop/src/shell/messages.tsx').read_text(encoding='utf-8')
required=set(re.findall(r'messageId:\s*e\?\.code\s*\?\?\s*[\'\"]([^\'\"]+)',source))
missing=required-known
print('Uncatalogued failure fallbacks:',sorted(missing))
assert not missing, 'front-end messages must have ui.help.get message content'
