import pathlib,json,re
E=pathlib.Path('reviews/evidence/M1-C1.4-r5')
for name in ['windows','macos','linux']:
 s=(E/f'ci-{name}.log').read_text(encoding='utf-16')
 assert '"passed": 24' in s,(name,'SDK')
 if name!='linux':
  for n in [23,14,5]: assert re.search(r'Ran '+str(n)+r' tests',s),(name,n)
  for test in ['group_accepts_a_matching_expect_rev','group_expect_rev_is_supported_and_conflicts_atomically','the_batch_tool_and_its_replayed_group_honour_expect_rev']:
   assert re.search(test+r' \.{3} ok',s),(name,test)
 print(name,': SDK 24/24;'+(' r1/r2/r3 23/14/5 and r4 group probes pass' if name!='linux' else ' no-display SDK execution verified'))
