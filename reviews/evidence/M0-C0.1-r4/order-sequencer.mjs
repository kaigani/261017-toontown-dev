export default class ReviewSequencer {
 shard(files) { return files; }
 sort(files) {
  const sorted=[...files].sort((a,b)=>a.moduleId.localeCompare(b.moduleId));
  if(process.env.REVIEW_REVERSE==='1') sorted.reverse();
  console.log('REVIEW FILE ORDER',sorted.map(f=>f.moduleId));
  return sorted;
 }
}
