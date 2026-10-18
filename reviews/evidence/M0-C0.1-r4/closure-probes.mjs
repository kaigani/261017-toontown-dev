import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync,mkdtempSync,readFileSync,writeFileSync,rmSync} from 'node:fs';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
const out=path.dirname(fileURLToPath(import.meta.url));
const desktop=path.resolve(out,'../../../desktop');
const require=createRequire(path.join(desktop,'package.json'));
const {ESLint}=require('eslint');
const {build,createServer,resolveConfig}=await import(pathToFileURL(require.resolve('vite')));
const eslint=new ESLint({cwd:desktop});
const parent=path.join(desktop,'src/__probes__');mkdirSync(parent,{recursive:true});
const dir=mkdtempSync(path.join(parent,'r4-'));
const configFile=path.join(desktop,'vite.config.ts');
const rows=[];
const record=(name,data)=>{rows.push({name,...data});console.log(JSON.stringify(rows.at(-1)));};
const chunks=r=>(Array.isArray(r)?r:[r]).flatMap(x=>x.output??[]).filter(x=>x.type==='chunk').map(x=>x.code).join('\n');
async function bundle(name,code,extra={}) {
 const entry=path.join(dir,name);writeFileSync(entry,code);
 try {const r=await build({root:desktop,configFile,logLevel:'silent',mode:'production',build:{write:false,sourcemap:false,lib:{entry,formats:['es'],fileName:'probe'},...extra}});return {accepted:true,code:chunks(r)};}
 catch(e){return {accepted:false,error:String(e.message)}}
}
try {
 for(const ext of ['ts','tsx','mts','cts','js','jsx','mjs','cjs']) {
  const config=await eslint.calculateConfigForFile(path.join(dir,'probe.'+ext));
  record('resolved-config-'+ext,{noInlineConfig:config?.linterOptions?.noInlineConfig,compat:config?.rules?.['compat/compat']});
  assert.equal(config.linterOptions.noInlineConfig,true);
 }
 const escaped=String.raw`'\u005f\u005fTAURI_INTERNALS__'`;
 for(const comment of ['/* eslint-env browser */','/* global __TAURI_INTERNALS__ */','/* exported __TAURI_INTERNALS__ */','/* eslint no-restricted-syntax: off */','// eslint-disable-line no-restricted-syntax']) {
  const [r]=await eslint.lintText(`${comment}\nexport const f = () => window[${escaped}];`,{filePath:path.join(dir,'comment.js')});
  record('inline '+comment,{errors:r.errorCount,messages:r.messages.map(m=>m.ruleId+': '+m.message)});assert.ok(r.errorCount);
 }
 for(const [name,code] of [
  ['template.js',String.raw`export const f=()=>window[\`\u005f\u005fTAURI_INTERNALS__\`];`.replaceAll('\\`','`')],
  ...['globalThis','self','frames','top'].map(alias=>[alias+'.js',`export const f=()=>${alias}[${escaped}];`]),
  ['reflect.js',`export const f=()=>Reflect.get(window,${escaped});`],
  ['concat.js',`export const f=()=>window['__TAU'+'RI_INTERNALS__'].invoke('app_info');`],
  ['chars.js',`export const f=()=>window[String.fromCharCode(${[...'__TAURI_INTERNALS__'].map(c=>c.charCodeAt(0)).join(',')})].invoke('app_info');`],
 ]) {
  const [lint]=await eslint.lintText(code,{filePath:path.join(dir,name)});
  const result=await bundle(name,code);
  record(name,{lintErrors:lint.errorCount,...result});
 }
 writeFileSync(path.join(dir,'name.json'),String.raw`{"name":"\u005f\u005fTAURI_INTERNALS__"}`);
 record('json-import',await bundle('json.js',`import data from './name.json';export const f=()=>window[data.name];`));
 writeFileSync(path.join(dir,'worker.js'),`import {invoke} from '@tauri-apps/api/core';self.onmessage=()=>invoke('app_info');`);
 const workerEntry=`export const f=()=>new Worker(new URL('./worker.js',import.meta.url),{type:'module'});`;
 record('worker-tauri',await bundle('worker-entry.js',workerEntry));
 const [workerLint]=await eslint.lintText(readFileSync(path.join(dir,'worker.js'),'utf8'),{filePath:path.join(dir,'worker.js')});
 record('worker-lint',{errors:workerLint.errorCount,messages:workerLint.messages});assert.ok(workerLint.errorCount);
 const tsOld=require('typescript');
 const oldSource=execFileSync('git',['show','642e4d3:desktop/boundary/vite-plugin.ts'],{cwd:desktop,encoding:'utf8'}).replace("'./eslint.js'",JSON.stringify(pathToFileURL(path.join(desktop,'boundary/eslint.js')).href));
 const oldCode=tsOld.transpileModule(oldSource,{compilerOptions:{module:tsOld.ModuleKind.ESNext,target:tsOld.ScriptTarget.ES2022}}).outputText;
 const old=await import('data:text/javascript;base64,'+Buffer.from(oldCode).toString('base64'));
 try {await build({root:desktop,configFile:false,logLevel:'silent',plugins:[old.ipcBoundary({allowTestSupport:false,desktopDir:desktop})],build:{write:false,target:['es2022','chrome110','safari15'],lib:{entry:path.join(dir,'worker-entry.js'),formats:['es'],fileName:'old-worker'}}});record('worker-before-r3-fixes',{accepted:true});}
 catch(e){record('worker-before-r3-fixes',{accepted:false,error:e.message});}
 record('manual-shared-concat',await bundle('shared.js',`import {createDefaultHostApi} from '../../ipc';export const good=()=>createDefaultHostApi().appInfo();export const bad=()=>window['__TAU'+'RI_INTERNALS__'];`,{rollupOptions:{output:{manualChunks:()=> 'shared'}}}));
 writeFileSync(path.join(dir,'reexport.js'),`export {createTauriTransport} from '../../ipc/tauriTransport.ts';`);
 for(const target of ['/src/ipc/tauriTransport.ts','/src/ipc/index.ts','/'+path.relative(desktop,path.join(dir,'reexport.js')).replaceAll('\\','/')]) {
  try {await build({root:desktop,configFile,logLevel:'silent',plugins:[{name:'r4-html',transformIndexHtml:{order:'pre',handler:()=>`<script type="module">import * as x from '${target}';window.probe=x;</script>`}}],build:{write:false}});record('html '+target,{accepted:true});}
  catch(e){record('html '+target,{accepted:false,error:e.message});}
 }
 const html=path.join(dir,'second.html');writeFileSync(html,`<script type="module">import {createTauriTransport} from '/src/ipc/tauriTransport.ts';window.b=createTauriTransport;</script>`);
 try {await build({root:desktop,configFile,logLevel:'silent',build:{write:false,rollupOptions:{input:html}}});record('second-html',{accepted:true});}
 catch(e){record('second-html',{accepted:false,error:e.message});}
 const c=await resolveConfig({root:desktop,configFile},'build');
 const scanner=c.plugins.find(p=>p.name==='toontown-ipc-boundary-scan');
 const hook=typeof scanner.renderChunk==='function'?scanner.renderChunk:scanner.renderChunk.handler;
 const mention=`window['__TAURI_INTERNALS__'];`;
 try{hook.call({error(m){throw Error(m)}},mention+mention,{fileName:'shared.js',modules:{[path.join(desktop,'src/ipc/tauriTransport.ts').replaceAll('\\','/')]:{code:mention},[path.join(desktop,'src/App.tsx').replaceAll('\\','/')]:{code:mention}}});record('shared-chunk-attribution',{accepted:true});}
 catch(e){record('shared-chunk-attribution',{accepted:false,error:e.message});}
 const server=await createServer({root:desktop,configFile,logLevel:'silent',server:{port:0,strictPort:false,ws:false}});
 try {
  await server.listen();const base=server.resolvedUrls.local[0];
  for(const url of ['/src/ipc/tauriTransport.ts','/','/src/main.tsx','/src/App.tsx','/src/ipc/index.ts','/src/ipc/tauriTransport.ts']) {
   const r=await fetch(new URL(url,base));const text=await r.text();record('dev '+url,{status:r.status,boundary:text.includes('IPC boundary'),bytes:text.length});
  }
 }finally{await server.close();}
 const [compat]=await eslint.lintText('export const x = structuredClone({});',{filePath:path.join(dir,'floor.js')});
 record('compat-probe',{messages:compat.messages});assert.ok(compat.messages.some(m=>m.ruleId==='compat/compat'));
 const ts=require('typescript');const source=readFileSync(path.join(desktop,'src/ipc/schema.ts'),'utf8');
 const code=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
 const {certify,guard,plainDataViolations}=await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'));
 let cases=0,falseAccepts=0,falseRejects=0;
 for(let n=0;n<=6;n++)for(let holes=0;holes<=n;holes++)for(let extras=0;extras<=6;extras++) {
  const a=Array(n).fill(1);for(let i=0;i<holes;i++)delete a[i];for(let i=0;i<extras;i++)a['extra'+i]=i;
  const accepted=plainDataViolations(a).length===0;cases++;
  if(accepted&&(holes||extras))falseAccepts++;if(!accepted&&!holes&&!extras)falseRejects++;
 }
 record('array-matrix',{cases,falseAccepts,falseRejects});assert.equal(falseAccepts+falseRejects,0);
 const original={nested:[{},[],[null,true,-0,{text:'x'.repeat(1048576)}]],empty:[]};
 const copied=certify(guard({}),original);assert.deepEqual(copied,original);assert.notEqual(copied.nested,original.nested);
 assert.ok(Object.is(copied.nested[2][2],-0));assert.ok(Object.is(structuredClone(original).nested[2][2],-0));
 original.empty.push(1);assert.deepEqual(copied.empty,[]);
 record('deep-copy',{nested:true,empty:true,negativeZeroPreservedLikeStructuredClone:true,largeStringBytes:1048576,snapshotIsolated:true});
} finally {
 writeFileSync(path.join(out,'closure-probes.json'),JSON.stringify(rows,null,2)+'\n');
 rmSync(dir,{recursive:true,force:true});
}
