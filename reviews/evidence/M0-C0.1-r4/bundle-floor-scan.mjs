import {readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {pathToFileURL,fileURLToPath} from 'node:url';
const require=createRequire(new URL('../../../desktop/package.json',import.meta.url));
const {parseAst}=await import(pathToFileURL(require.resolve('vite')));
const dist=new URL('../../../desktop/dist/assets/',import.meta.url);
const patterns=['structuredClone','Object\\.hasOwn','Array\\.prototype\\.at','\\.at\\(','\\?\\?=','\\|\\|=','Object\\.groupBy','Array\\.prototype\\.findLast','\\.findLast\\(','replaceAll','Promise\\.any','AggregateError','WeakRef','Intl\\.Segmenter','class\\s+[^{}]*\\{\\s*static\\s*\\{','#[A-Za-z_$]','\\(\\?<([=!])'];
const rows=[];
for(const file of readdirSync(dist).filter(f=>f.endsWith('.js'))) {
 const code=readFileSync(new URL(file,dist),'utf8');
 const hits={};for(const pattern of patterns) {
  const matches=[...code.matchAll(new RegExp(pattern,'g'))];hits[pattern]={count:matches.length,contexts:matches.slice(0,8).map(m=>code.slice(Math.max(0,m.index-90),m.index+150))};
 }
 const syntax={staticBlocks:0,privateIdentifiers:0,topLevelAwait:0,lookbehind:0};
 function walk(n,inFunction=false){
  if(!n||typeof n!=='object')return;
  if(Array.isArray(n)){for(const c of n)walk(c,inFunction);return;}
  if(n.type==='StaticBlock')syntax.staticBlocks++;
  if(n.type==='PrivateIdentifier')syntax.privateIdentifiers++;
  if(n.type==='AwaitExpression'&&!inFunction)syntax.topLevelAwait++;
  if(n.regex&&/\(\?<([=!])/.test(n.regex.pattern))syntax.lookbehind++;
  const fn=inFunction||/Function/.test(n.type??'');
  for(const [k,v]of Object.entries(n))if(k!=='type'&&typeof v==='object')walk(v,fn);
 }
 walk(parseAst(code));rows.push({file,bytes:code.length,hits,syntax});
}
writeFileSync(new URL('./bundle-floor-scan.json',import.meta.url),JSON.stringify(rows,null,2)+'\n');
console.log(JSON.stringify(rows,null,2));
