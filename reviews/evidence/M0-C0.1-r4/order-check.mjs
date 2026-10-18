import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import ReviewSequencer from './order-sequencer.mjs';
const root=fileURLToPath(new URL('../../../desktop/',import.meta.url));
const require=createRequire(new URL('../../../desktop/package.json',import.meta.url));
const {startVitest}=await import(pathToFileURL(require.resolve('vitest/node')));
const ctx=await startVitest('test',['src/test/c01-r3-schema.test.ts','src/ipc/hostApi.test.ts'],{
 root,config:root+'vite.config.ts',run:true,reporters:['verbose'],sequence:{sequencer:ReviewSequencer},
});
console.log('RESOLVED fsModuleCache',ctx.config.fsModuleCache);
if(ctx.config.fsModuleCache!==false)process.exitCode=1;
await ctx.close();
