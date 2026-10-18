import {createRequire} from 'node:module';
import {mkdirSync,mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
const desktop=fileURLToPath(new URL('../../../desktop/',import.meta.url));
const require=createRequire(path.join(desktop,'package.json'));
const {build}=await import(pathToFileURL(require.resolve('vite')));
const parent=path.join(desktop,'src/__probes__');mkdirSync(parent,{recursive:true});
const dir=mkdtempSync(path.join(parent,'r4-app-'));
const source='export const bypass=()=>window[String.fromCharCode(95,95,84,65,85,82,73,95,73,78,84,69,82,78,65,76,83,95,95)].invoke("review_raw_probe");';
const entry=path.join(dir,'probe.js');writeFileSync(entry,source);
const observations=[];
try{
 const result=await build({root:desktop,configFile:path.join(desktop,'vite.config.ts'),mode:'production',logLevel:'silent',plugins:[{
  name:'review-main-fixture',
  transform(code,id){if(id.replaceAll('\\','/')===path.join(desktop,'src/main.tsx').replaceAll('\\','/'))return `import {bypass} from '${'./'+path.relative(path.join(desktop,'src'),entry).replaceAll('\\','/')}';window.reviewBypass=bypass;\n`+code;},
  renderChunk:{order:'post',handler(code){const i=code.indexOf('review_raw_probe');if(i>=0)observations.push({stage:'renderChunk order post',snippet:code.slice(i-170,i+90)});return null;}},
 }],build:{write:false,sourcemap:false}});
 for(const c of (Array.isArray(result)?result:[result]).flatMap(r=>r.output??[]).filter(c=>c.type==='chunk')){
  const i=c.code.indexOf('review_raw_probe');if(i>=0)observations.push({stage:'final app bundle',snippet:c.code.slice(i-120,i+80)});
 }
 observations.push({buildAccepted:true});
}catch(e){observations.push({buildAccepted:false,error:String(e)});}
finally{rmSync(dir,{recursive:true,force:true});}
writeFileSync(new URL('./late-fold-app.json',import.meta.url),JSON.stringify(observations,null,2)+'\n');
console.log(JSON.stringify(observations,null,2));
