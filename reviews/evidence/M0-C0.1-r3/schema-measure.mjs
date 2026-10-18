import {createRequire} from 'node:module';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
const require=createRequire(new URL('../../../desktop/package.json',import.meta.url));
const ts=require('typescript');
const source=readFileSync(new URL('../../../desktop/src/ipc/schema.ts',import.meta.url),'utf8');
const code=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const {certify,guard,plainDataViolations}=await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'));
const response={seq:1,payload:'x'.repeat(1048576),engineUptimeNs:1,handlerNs:1};
const check=guard({type:'object',properties:{seq:{type:'integer',minimum:0,maximum:Number.MAX_SAFE_INTEGER},payload:{type:'string'},engineUptimeNs:{type:'integer'},handlerNs:{type:'integer'}},required:['seq','payload','engineUptimeNs','handlerNs'],additionalProperties:false});
const timings=[];
for(let i=0;i<110;i++) {const start=performance.now();const result=certify(check,response);if(!result)throw new Error('rejected valid string');if(i>=10)timings.push(performance.now()-start);}
timings.sort((a,b)=>a-b);
console.log('1MiB certify, 10 warmups + 100 iterations, ms', {p50:timings[50],p95:timings[95],max:timings[99]});
let cases=0;let falseAccepts=0;
for(let length=0;length<=6;length++)for(let holes=0;holes<=length;holes++)for(let extras=0;extras<=6;extras++) {
  const arr=Array(length).fill(1);for(let i=0;i<holes;i++)delete arr[i];for(let i=0;i<extras;i++)arr['extra'+i]=i;
  const accepted=plainDataViolations(arr).length===0;
  cases++;
  if(accepted && (holes || extras)){falseAccepts++;console.log('false accept',{length,holes,extras});}
  if(accepted !== (holes===extras))throw new Error('unexpected matrix result');
}
console.log('array property matrix',{cases,falseAccepts});
console.log('source',fileURLToPath(new URL('../../../desktop/src/ipc/schema.ts',import.meta.url)));
