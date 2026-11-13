import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { startHarness, layoutProbe, root, sleep } from "./ui-harness.mjs";

const evidence = resolve(root, "design/screens");
mkdirSync(evidence, { recursive: true });

const fixture = `
(()=>{const q=new URLSearchParams(location.search);if(q.has('theme'))localStorage.setItem('karps.theme',q.get('theme'));if(q.has('lang'))localStorage.setItem('karps.lang',q.get('lang'));})();
window.calls=[];
const KEEP=new URLSearchParams(location.search).has('keep');
const saved=KEEP?JSON.parse(localStorage.getItem('fx.state')||'null'):null;
if(!KEEP)localStorage.removeItem('fx.state');
window.store=saved?saved.store:{};window.pstore=saved?saved.pstore:{};let pid=saved?saved.pid:0;
const persist=()=>localStorage.setItem('fx.state',JSON.stringify({store,pstore,pid}));
let tick=1767225600000;const now=()=>(tick+=60000);
const pmeta=(p)=>JSON.parse(JSON.stringify({id:p.id,name:p.name,created:p.created,updated:p.updated,game:p.game,gamePath:p.gamePath,gameId:p.gameId,characters:p.characters}));
window.pack=[];window.committed=new Set();window.lastLoaded=null;
window.scanMode=new URLSearchParams(location.search).get('discovery')||'empty';window.scanWaiters=[];
function discoveredFixture(){return {games:[{path:'D:/Games/Mario Kart Wii (USA).wbfs',id:'RMCE01',region:'USA',format:'WBFS',sizeBytes:2800000000},{path:'E:/Wii/Mario Kart Wii (Europe).iso',id:'RMCP01',region:'Europe',format:'ISO',sizeBytes:4700000000}],truncated:window.scanMode==='limited'};}
const NAMES=[['bmr','Baby Mario'],['blg','Baby Luigi'],['bpc','Baby Peach'],['bds','Baby Daisy'],['ko','Toad'],['kk','Toadette'],['nk','Koopa Troopa'],['ka','Dry Bones'],['mr','Mario'],['lg','Luigi'],['pc','Peach'],['ds','Daisy'],['ys','Yoshi'],['ca','Birdo'],['dd','Diddy Kong'],['jr','Bowser Jr.'],['wr','Wario'],['wl','Waluigi'],['dk','Donkey Kong'],['kp','Bowser'],['kt','King Boo'],['rs','Rosalina'],['fk','Funky Kong'],['bk','Dry Bowser']];
function synth(w,h,hue){const c=document.createElement('canvas');c.width=w;c.height=h;const g=c.getContext('2d');
 const gr=g.createLinearGradient(0,0,w,h);gr.addColorStop(0,'hsl('+hue+' 80% 55%)');gr.addColorStop(1,'hsl('+(hue+60)+' 70% 35%)');g.fillStyle=gr;g.fillRect(0,0,w,h);
 g.fillStyle='rgba(255,255,255,1)';for(let x=0;x<w;x+=16)for(let y=(x/16)%2*16;y<h;y+=32)g.fillRect(x,y,8,8);g.clearRect(w*0.8,0,w*0.2,h*0.25);return c.toDataURL();}
const TEX=[['mario_all',128,64,false,true],['mario_tx_64',64,64,false,false]].concat([0,1,2,3].map(i=>['mario_eye.'+i,32,64,true,i<2]));
const BIKE=[['bike_suit_all',128,64,false,true],['bike_suit_tx_6432',32,64,false,false]];
const texCache={};const tex=(code)=>texCache[code]||(texCache[code]=TEX.concat(code==='pc'?BIKE:[]).map(([name,width,height,eye,menu],i)=>({name,width,height,format:'CMPR',images:1,files:48,menu,eye,preview:synth(width,height,i*40),sha256:'synthetic-'+name})));
const b64=(ta)=>{const u=new Uint8Array(ta.buffer,ta.byteOffset,ta.byteLength);let s='';for(let i=0;i<u.length;i+=0x8000)s+=String.fromCharCode(...u.subarray(i,i+0x8000));return btoa(s);};
function quad(x0,y0,x1,y1,z,uv,parts){const [u0,v0,u1,v1]=uv;return {pos:[x0,y1,z,x1,y1,z,x1,y0,z,x0,y0,z],nrm:[0,0,1,0,0,1,0,0,1,0,0,1],uvs:[u0,v0,u1,v0,u1,v1,u0,v1],idx:[0,1,2,0,2,3],parts};}
function merge(texture,quads){const P=[],N=[],U=[],I=[],T=[];let base=0;for(const q of quads){P.push(...q.pos);N.push(...q.nrm);U.push(...q.uvs);I.push(...q.idx.map(i=>i+base));T.push(...q.parts);base+=4;}
 return {material:texture+'_tx',shape:'polygon',texture,wrapS:1,wrapT:1,xlu:false,vertices:base,triangles:I.length/3,positions:b64(new Float32Array(P)),normals:b64(new Float32Array(N)),uvs:b64(new Float32Array(U)),indices:b64(new Uint32Array(I)),parts:b64(new Uint16Array(T))};}
function rectRuns(w,x0,y0,x1,y1){const r=[];for(let y=y0;y<y1;y++)r.push(y*w+x0,x1-x0);return r;}
function part(key,n,w,x0,y0,x1,y1){return {key,n,material:'synthetic',pixels:(x1-x0)*(y1-y0),bbox:[x0,y0,x1,y1],runs:b64(new Uint32Array(rectRuns(w,x0,y0,x1,y1))),outline:b64(new Uint16Array([x0,y0,x1,y0,x0,y1,x1,y1,x0,y0,x0,y1,x1,y0,x1,y1]))};}
function runsExcept(w,h,x0,y0,x1,y1){const r=[];for(let y=0;y<h;y++){if(y>=y0&&y<y1){r.push(y*w,x0);if(x1<w)r.push(y*w+x1,w-x1);}else r.push(y*w,w);}return r;}
function lowSamples(){const s=new Int32Array(64*64*4).fill(-1);for(let y=0;y<64;y++)for(let x=0;x<32;x++)for(let k=0;k<4;k++)s[(y*64+x)*4+k]=(y*128+2*x+(k&1));return s;}
function lowSamples32(){const s=new Int32Array(32*64*4).fill(-1);for(let y=0;y<64;y++)for(let x=0;x<32;x++)for(let k=0;k<4;k++)s[(y*32+x)*4+k]=(y*128+2*x+(k&1));return s;}
function facts(){const all=(w,h)=>b64(new Uint32Array([0,w*h]));
 return [{texture:'mario_all',width:128,height:64,role:'body',usedTexels:128*64-32*16,mirroredTexels:0,smearTriangles:0,lods:['race1'],derivedFrom:null,used:b64(new Uint32Array(runsExcept(128,64,96,0,128,16)))},
  {texture:'mario_tx_64',width:64,height:64,role:'low',usedTexels:4096,mirroredTexels:0,smearTriangles:0,lods:['race2'],derivedFrom:'mario_all',used:all(64,64)}]
  .concat([0,1,2,3].map(i=>({texture:'mario_eye.'+i,width:32,height:64,role:'eye',usedTexels:2048,mirroredTexels:0,smearTriangles:0,lods:['race1'],derivedFrom:null,used:all(32,64)})));}
function bundle(code){
 const body=merge('mario_all',[quad(-60,0,-1,100,0,[0,0,0.5,1],[0,0]),quad(1,0,60,100,0,[0.5,0,1,1],[1,1]),quad(-40,-50,0,-5,0,[0.25,0.6,0.375,0.9],[0,0]),quad(0,-50,40,-5,0,[0.875,0.6,1,0.9],[1,1])]);
 const eye=merge('mario_eye.0',[quad(-20,70,20,90,4,[0,0,1,1],[0,0])]);
 const textures=[{texture:'mario_all',width:128,height:64,model:0,aliasOf:null,parts:[part('head',0,128,0,0,64,64),part('torso',0,128,64,0,128,64)]},
  {texture:'mario_tx_64',width:64,height:64,model:null,aliasOf:null,parts:[]}]
  .concat([0,1,2,3].map(i=>({texture:'mario_eye.'+i,width:32,height:64,model:0,aliasOf:i?'mario_eye.0':null,parts:[part('eyes',0,32,0,0,32,64)]})));
 const b={character:code,models:[{kind:'race',file:'Race/Kart/x-mr.szs',name:'model',bboxMin:[-60,-50,0],bboxMax:[60,100,4],submeshes:[body,eye]}],textures,unreferenced:['mario_tx_64'],
  facts:facts(),links:[{low:'mario_tx_64',width:64,height:64,highs:[{texture:'mario_all',width:128,height:64}],samples:b64(lowSamples()),mapped:2048,distRel:0.01}],
  eyes:[{frames:[0,1,2,3].map(i=>'mario_eye.'+i),defaultFrame:'mario_eye.0',blink:['mario_eye.0','mario_eye.1','mario_eye.2','mario_eye.1','mario_eye.0']}]};
 b.models[0].vehicle='kart';b.models[0].detail='full';
 if(code==='pc'){
  const low=merge('mario_tx_64',[quad(-30,10,30,70,0,[0,0,0.5,1],[0,0])]);
  const bikeBody=merge('bike_suit_all',[quad(-60,0,-1,100,0,[0,0,0.5,1],[0,0]),quad(1,0,60,100,0,[0.5,0,1,1],[1,1])]);
  const bikeLow=merge('bike_suit_tx_6432',[quad(-30,10,30,70,0,[0,0,1,1],[0,0])]);
  b.models[0].file='Race/Kart/ma_kart-pc.szs';
  b.models.push({kind:'race',file:'Race/Kart/ma_kart-pc.szs',name:'model_lod',vehicle:'kart',detail:'low',bboxMin:[-30,10,0],bboxMax:[30,70,1],submeshes:[low]},
   {kind:'race',file:'Race/Kart/ma_bike-pc.szs',name:'model',vehicle:'bike',detail:'full',bboxMin:[-60,-50,0],bboxMax:[60,100,4],submeshes:[bikeBody,eye]},
   {kind:'race',file:'Race/Kart/ma_bike-pc.szs',name:'model_lod',vehicle:'bike',detail:'low',bboxMin:[-30,10,0],bboxMax:[30,70,1],submeshes:[bikeLow]});
  const lowT=b.textures.find(t=>t.texture==='mario_tx_64');lowT.model=1;lowT.parts=[part('head',0,64,0,0,32,64)];
  b.textures.push({texture:'bike_suit_all',width:128,height:64,model:2,aliasOf:null,parts:[part('dress',0,128,0,0,64,64),part('legs',0,128,64,0,128,64)]},{texture:'bike_suit_tx_6432',width:32,height:64,model:3,aliasOf:null,parts:[part('torso',0,32,0,0,32,64)]});
  b.unreferenced=[];
  b.facts.push({texture:'bike_suit_all',width:128,height:64,role:'body',usedTexels:8192,mirroredTexels:0,smearTriangles:0,lods:['race1'],derivedFrom:null,used:b64(new Uint32Array([0,8192]))},{texture:'bike_suit_tx_6432',width:32,height:64,role:'low',usedTexels:2048,mirroredTexels:0,smearTriangles:0,lods:['race1'],derivedFrom:'bike_suit_all',used:b64(new Uint32Array([0,2048]))});
  b.links.push({low:'bike_suit_tx_6432',width:32,height:64,highs:[{texture:'bike_suit_all',width:128,height:64}],samples:b64(lowSamples32()),mapped:2048,distRel:0.01});
  const shareS=()=>{const s=new Int32Array(128*64*4).fill(-1);for(let y=0;y<24;y++)for(let x=0;x<64;x++)for(let k=0;k<4;k++)s[(y*128+x)*4+k]=y*128+x;return s;};
  b.shares=[{low:'bike_suit_all',width:128,height:64,highs:[{texture:'mario_all',width:128,height:64}],samples:b64(shareS()),mapped:1536,distRel:0},
   {low:'mario_all',width:128,height:64,highs:[{texture:'bike_suit_all',width:128,height:64}],samples:b64(shareS()),mapped:1536,distRel:0}];
 }
 if(window.noEyes){b.models[0].submeshes=[body];b.eyes=[];b.facts=b.facts.filter(f=>f.role!=='eye');}
 return b;}
function importImage(){const c=document.createElement('canvas');c.width=40;c.height=20;const g=c.getContext('2d');g.fillStyle='#00ff00';g.fillRect(0,0,40,20);return c.toDataURL();}
let cb=0;
window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main',windowLabel:'main'}},transformCallback(){return ++cb},unregisterCallback(){},convertFileSrc:s=>s,
 async invoke(cmd,args){calls.push({cmd,args});
  if(cmd==='plugin:window|set_theme')return null;
  if(cmd==='plugin:dialog|open'){const f=args.options&&args.options.filters&&args.options.filters[0];if(args.options&&args.options.directory)return 'D:/Games/My game copy/DATA/files';if(f&&f.extensions.includes('mkss'))return 'D:/Skins/test.mkss';if(f&&f.extensions.includes('wbfs'))return window.cancelPicker?null:'D:/Games/Manual game.wbfs';return 'D:/Art/green.png';}
  if(cmd==='plugin:dialog|save'){const f=args.options.filters&&args.options.filters[0];return f&&f.extensions.includes('mkss')?'D:/Skins/copy.mkss':'D:/Skins/'+(args.options.defaultPath||'out.zip');}
  if(cmd==='plugin:event|listen')return 1;
  if(cmd==='plugin:event|unlisten')return null;
  if(cmd==='plugin:opener|reveal_item_in_dir')return null;
  if(cmd==='plugin:opener|open_url')return null;
  if(cmd==='scan_for_game'){if(window.scanMode==='deferred')return new Promise(resolve=>scanWaiters.push(resolve));if(window.scanMode==='error')throw 'E:game.scan';if(window.scanMode==='single')return {games:discoveredFixture().games.slice(0,1),truncated:false};return window.scanMode==='found'||window.scanMode==='limited'?discoveredFixture():{games:[],truncated:false};}
  if(cmd==='open_game'&&window.failOpen)throw 'E:game.wrongGame';
  if(cmd==='open_game')return {root:args.path,id:/\.(wbfs|iso)$/i.test(args.path)?'RMCE01':null,source:args.path.toLowerCase().endsWith('.iso')||args.path.toLowerCase().endsWith('.wbfs')?'image':'folder',characters:NAMES.map(([code,name],i)=>({code,name,files:['wr','wl','dk','kp','kt','rs','fk','bk'].includes(code)?47:48,portrait:window.missingPortraits?null:window.brokenPortraits?'data:image/png;base64,AAAA':synth(64,64,i*15)}))};
  if(cmd==='load_character'){window.lastLoaded=args.code;window.committed=new Set();}
  if(cmd==='load_character')return {code:args.code,name:NAMES.find(n=>n[0]===args.code)[1],files:48,menu:true,textures:tex(args.code)};
  if(cmd==='model_load'){if(window.failModel)throw 'E:model.none';return bundle(args.character);}
  if(cmd==='read_image')return {name:'green.png',width:40,height:20,sha256:'abc',dataUrl:importImage()};
  if(cmd==='commit_texture'){if(args.png)committed.add(args.name);else committed.delete(args.name);return !!args.png;}
  const ORDER=NAMES.map(n=>n[0]);
  if(cmd==='projects_list')return Object.values(pstore).sort((a,b)=>b.updated-a.updated).map(pmeta);
  if(cmd==='project_create'){if(!args.name.trim())throw 'E:project.name';const id='p'+(++pid);pstore[id]={id,name:args.name.trim(),created:now(),updated:tick,game:args.game,gamePath:args.gamePath,gameId:args.gameId,characters:[],data:{}};persist();return pmeta(pstore[id]);}
  if(cmd==='project_set_game'){const p=pstore[args.id];Object.assign(p,{game:args.game,gamePath:args.gamePath,gameId:args.gameId,updated:now()});persist();return pmeta(p);}
  if(cmd==='project_rename'){const p=pstore[args.id];p.name=args.name;p.updated=now();persist();return pmeta(p);}
  if(cmd==='project_duplicate'){const id='p'+(++pid);pstore[id]=JSON.parse(JSON.stringify(pstore[args.id]));Object.assign(pstore[id],{id,name:args.name,created:now(),updated:tick});persist();return pmeta(pstore[id]);}
  if(cmd==='project_delete'){delete pstore[args.id];persist();return null;}
  if(cmd==='project_export_file'){store[args.path]={copyOf:args.id};return 4321;}
  if(cmd==='project_import'){const f=store[args.path];if(!f)throw 'E:project.read';const m=JSON.parse(f.manifest);if(m.format!=='mkss-project/1.1')throw 'E:project.format';const id='p'+(++pid);
   pstore[id]={id,name:args.name,created:now(),updated:tick,game:'',gamePath:'',gameId:'',characters:[{code:m.character.code,name:m.character.name,savedAt:tick,edits:[]}],data:{[m.character.code]:f}};persist();return {meta:pmeta(pstore[id]),existing:false};}
  if(cmd==='take_open_requests'){return [];}
  if(cmd==='project_char_read'){const p=pstore[args.id];if(!p)throw 'E:project.unknown';return p.data[args.code]||null;}
  if(cmd==='project_char_write'){const p=pstore[args.id];const has=!!args.project&&JSON.parse(args.project.manifest).textures.length>0;p.characters=p.characters.filter(c=>c.code!==args.code);delete p.data[args.code];
   if(has){p.data[args.code]=JSON.parse(JSON.stringify(args.project));p.characters.push({code:args.code,name:args.name,savedAt:now(),edits:args.edits});p.characters.sort((a,b)=>ORDER.indexOf(a.code)-ORDER.indexOf(b.code));}p.updated=now();persist();return pmeta(p);}
  if(cmd==='autosave_write'){store['auto:'+args.id+':'+args.code]=args.project;persist();return null;}
  if(cmd==='autosave_read')return window.offerAutosave?store['auto:'+args.id+':'+args.code]||null:null;
  if(cmd==='autosave_clear'){delete store['auto:'+args.id+':'+args.code];persist();return null;}
  if(cmd==='pack_begin'){window.pack=[];return null;}
  if(cmd==='pack_stage'){window.pack=pack.filter(x=>x.code!==lastLoaded);if(committed.size)pack.push({code:lastLoaded,textures:[...committed].sort()});return committed.size;}
  if(cmd==='pack_file_name')return args.name.replace(/[^A-Za-z0-9 _.-]/g,' ').trim()+'.zip';
  if(cmd==='build_pack'){if(!pack.length)throw 'E:build.nothing';return {path:args.out,bytes:15192685,files:pack.length*48+1,textures:pack.reduce((a,x)=>a+x.textures.length,0)};}
  if(cmd==='plugin:window|close'||cmd==='plugin:window|destroy'){window.closedWindow=(window.closedWindow||0)+1;return null;}
  if(cmd==='export_original')return null;
  throw Error('Unexpected fixture IPC '+cmd);
 }};
window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
window.confirm=()=>true;
`;

const H = await startHarness({ fixture });
const { value, wait, click, size, drag, mouse, send } = H;
const checks = [];
const shots = [];
function check(name, pass, detail) {
  assert.ok(pass, `${name}: ${JSON.stringify(detail)}`);
  checks.push(name);
}
async function shot(name) {
  await H.shot(resolve(evidence, name + ".png"));
  shots.push(`design/screens/${name}.png`);
}
const E = "window.__mkss.editor";
async function texel(x, y) {
  return value(`(()=>{const v=${E}.view;const r=v.canvas.getBoundingClientRect();return [r.left+v.ox+(${x}+0.5)*v.zoom, r.top+v.oy+(${y}+0.5)*v.zoom];})()`);
}
const px = (x, y) => value(`(()=>{const d=${E}.current;const a=window.__mkss.composite(d);const o=(${y}*d.w+${x})*4;return Array.from(a.slice(o,o+4));})()`);
const hashComposite = (name) => value(`(()=>{const d=${name ? `${E}.docs.get('${name}')` : `${E}.current`};const c=window.__mkss.composite(d);let h=0;for(let i=0;i<c.length;i++)h=(h*31+c[i])>>>0;return h;})()`);
const key = async (k, code, vk, modifiers = 0) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, modifiers, windowsVirtualKeyCode: vk });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, modifiers, windowsVirtualKeyCode: vk });
  await sleep(100);
};

async function boot(theme, lang, w, h, discovery='empty', keep=false) {
  await size(w, h);
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: theme }] });
  await send("Page.navigate", { url: `${H.origin}ui-test?theme=${theme}&lang=${lang}&discovery=${discovery}${keep ? '&keep=1' : ''}` });
  await wait("document.documentElement?.dataset.ready==='1'", 600);
  await sleep(200);
}
const S = "window.__mkss.state";
const tab = (t) => wait(`${S}.tab==='${t}'`);
async function toNew() {
  if ((await value(`${S}.tab`)) !== 'projects') await click(".app-tabs [data-tab=projects]");
  await value("(document.getElementById('proj-empty').classList.contains('hidden')?document.getElementById('btn-new-project'):document.getElementById('btn-new-project-empty')).click()");
  await tab('new');
  await sleep(80);
}
async function newProject(name) {
  await toNew();
  if (name) await value(`document.getElementById('new-name').value=${JSON.stringify(name)}`);
  if (!(await value("document.getElementById('game-alternatives').open"))) await click("#game-alternatives summary");
  await click("#btn-open-folder");
  await tab('characters');
  await sleep(100);
}
async function openChar(code) {
  await click(`#char-grid button.char[data-code=${code}]`);
  await wait(`${S}.tab==='studio' && ${E}.current && ${E}.character.code==='${code}' && ${E}.bundle && document.querySelectorAll('#parts-list li[data-i]').length>=1`);
  await sleep(300);
}
async function toEditor(code = 'mr') {
  await newProject();
  await openChar(code);
}
const askOpen = () => value("document.getElementById('ask-dialog').open");
async function answer(id) {
  await wait("document.getElementById('ask-dialog').open");
  await click(`#ask-actions [data-id=${id}]`);
  await wait("!document.getElementById('ask-dialog').open");
  await sleep(150);
}
async function leaveTo(t, choice = 'discard') {
  await click(t === "characters" ? "#btn-done" : `.app-tabs [data-tab=${t}]`);
  if (await askOpen()) await answer(choice);
  await tab(t);
}
async function toAdvanced() {
  await click("#mode-2d");
  await wait(`${E}.mode==='2d' && document.getElementById('view-edit').dataset.mode==='2d'`);
  await sleep(200);
}
const legalVisible = `(()=>[...document.querySelectorAll('.legal')].some(e=>{const r=e.getBoundingClientRect();const p=e.parentElement.getBoundingClientRect();return r.height>0&&r.width>0&&r.bottom<=innerHeight&&r.right<=Math.min(innerWidth,p.right)+0.5&&e.scrollWidth<=e.clientWidth+1&&e.textContent.length>5}))()`;

async function controlChrome() {
  const centered = (id) => `(()=>{const b=document.getElementById('${id}'),s=b.querySelector('svg');if(!s)return false;const r=b.getBoundingClientRect(),q=s.getBoundingClientRect(),p=s.querySelector('path').getBBox(),v=s.viewBox.baseVal;return q.width>=12 && Math.abs(r.left+r.width/2-q.left-q.width/2)<.5 && Math.abs(r.top+r.height/2-q.top-q.height/2)<.5 && Math.abs(p.x+p.width/2-v.width/2)<.1 && Math.abs(p.y+p.height/2-v.height/2)<.1 && s.getAttribute('aria-hidden')==='true' && b.textContent.trim().length>0})()`;
  for(const theme of ['light','dark']) for(const lang of ['en','pt','es','fr','ja']) {
    await boot(theme,lang,1280,720);
    await click('#btn-settings');
    for(const zoom of [1,1.25,1.5]) {
      await value(`document.getElementById('settings-dialog').style.zoom='${zoom}'`);
      check(`${theme}/${lang} modal close is visually centered at ${zoom}`,await value(centered('btn-settings-close')),'symmetric SVG path, not a baseline-positioned text glyph');
    }
    await value("document.getElementById('settings-dialog').style.zoom='1'");
    const arrow = await value("(()=>{const t=getComputedStyle(document.getElementById('opt-theme')),l=getComputedStyle(document.querySelector('.lang-btn'));return {appearance:t.appearance,image:t.backgroundImage,langImage:l.backgroundImage,position:t.backgroundPosition,langPosition:l.backgroundPosition,size:t.backgroundSize,langSize:l.backgroundSize,padding:parseFloat(t.paddingRight)}})()");
    check(`${theme}/${lang} Theme uses the same inset dropdown arrow as Language`,arrow.appearance==='none'&&arrow.image!=='none'&&arrow.image===arrow.langImage&&arrow.position===arrow.langPosition&&arrow.size===arrow.langSize&&arrow.padding>=26,arrow);
    const selects=await value("[...document.querySelectorAll('select')].map(e=>{const s=getComputedStyle(e);return {id:e.id,appearance:s.appearance,image:s.backgroundImage,position:s.backgroundPosition,padding:parseFloat(s.paddingRight)}})");
    check(`${theme}/${lang} all dropdowns reserve arrow space`,selects.length>=4&&selects.every(s=>s.appearance==='none'&&s.image===arrow.image&&s.position===arrow.position&&s.padding>=26),selects);
    if(lang==='en') {
      await send('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:'active'}]});
      check(`${theme} high contrast keeps native select and visible Language indicator`,await value("getComputedStyle(document.getElementById('opt-theme')).appearance==='auto'&&getComputedStyle(document.getElementById('opt-theme')).backgroundImage==='none'&&getComputedStyle(document.querySelector('.lang-caret')).display!=='none'"),'native select fallback, language text triangle, currentColor close SVG');
      await send('Emulation.setEmulatedMedia',{features:[]});
    }
    const pos=await value("(()=>{const r=document.querySelector('.lang-btn').getBoundingClientRect();return [r.left+r.width/2,r.top+r.height/2]})()");
    await mouse('mouseMoved',pos[0],pos[1]);
    check(`${theme}/${lang} dropdown arrow survives hover`,await value("getComputedStyle(document.querySelector('.lang-btn')).backgroundImage===getComputedStyle(document.getElementById('opt-theme')).backgroundImage"),'generic button hover must not remove background icon');
    if(lang==='en')await shot(`12-controls-${theme}-en`);
    await value("document.getElementById('opt-theme').focus()");
    await key('Home','Home',36);await key('ArrowDown','ArrowDown',40);
    check(`${theme}/${lang} native Theme still works with keyboard`,await value("document.getElementById('opt-theme').value==='light' && localStorage.getItem('karps.theme')==='light'"),'keep native select behavior');
    await click('#btn-settings-close');
    check(`${theme}/${lang} centered X closes and returns focus`,await value("!document.getElementById('settings-dialog').open && document.activeElement.id==='btn-settings'"),'click SVG bubbles to button handler');
    await click('#btn-tour');
    check(`${theme}/${lang} tutorial uses the same centered close icon`,await value(centered('tour-close')),'all modal X buttons share a component style');
    await click('#tour-close');
  }
}

async function sliderDesign() {
  await boot('dark','en',1280,720);
  await toEditor();
  await click('[data-tool=brush]');
  check('editor sliders use custom thin-track styling and full row width',await value("[...document.querySelectorAll('#tool-opts input[type=range],#lay-opacity')].every(e=>{const s=getComputedStyle(e),r=e.getBoundingClientRect(),p=e.parentElement.getBoundingClientRect(),label=e.parentElement.querySelector('span').getBoundingClientRect();return s.appearance==='none' && s.getPropertyValue('--range-track-size').trim()==='2px' && s.getPropertyValue('--range-thumb-size').trim()==='10px' && r.width>=p.width*.9 && r.top>=label.bottom})"),'label/value above a thin full-width range');
  const input=await value("(()=>{const e=document.getElementById('o-size3d'),r=e.getBoundingClientRect();return {x:r.x,y:r.y,w:r.width,h:r.height,value:Number(e.value)}})()");
  await value("document.getElementById('o-size3d').focus()");
  await key('ArrowRight','ArrowRight',39);
  check('custom slider retains native keyboard input and updates progress',await value(`Number(document.getElementById('o-size3d').value)===${input.value+1} && window.__mkss.editor.opts.size3d.brush===${input.value+1} && parseFloat(document.getElementById('o-size3d').style.getPropertyValue('--range-progress'))>0`),'native input is not replaced by a cosmetic div');
  const rect=await value("(()=>{const r=document.getElementById('lay-opacity').getBoundingClientRect();return [r.left,r.top,r.width,r.height]})()");
  await mouse('mousePressed',rect[0]+rect[2]*.4,rect[1]+rect[3]/2);
  await mouse('mouseReleased',rect[0]+rect[2]*.4,rect[1]+rect[3]/2);
  check('layer opacity drag updates actual pixels property and slider fill',await value("(()=>{const e=document.getElementById('lay-opacity'),d=window.__mkss.editor.current,l=d.layers.find(l=>l.id===d.active);return Number(e.value)>20 && Number(e.value)<60 && Math.abs(l.opacity*100-Number(e.value))<.1 && Math.abs(parseFloat(e.style.getPropertyValue('--range-progress'))-Number(e.value))<.1})()"),'same behavior with redesigned control');
  await click('#btn-undo');
  check('undo resynchronizes slider value and fill',await value("document.getElementById('lay-opacity').value==='100' && document.getElementById('lay-opacity').style.getPropertyValue('--range-progress')==='100%'"),'no stale progress after programmatic changes');
  await shot('11-sliders-dark-en');
}

async function functional() {
  await boot("light", "en", 1280, 720);
  await toNew();
  check('manual input is subtle and extracted folder is collapsed', await value("!document.getElementById('btn-open-image').classList.contains('primary') && !!document.querySelector('details#game-alternatives') && !document.getElementById('game-alternatives').open && document.getElementById('btn-open-folder').getBoundingClientRect().height===0"), 'secondary folder hidden by default');
  await click('#game-alternatives summary');
  check('extracted folder remains accessible under other options', await value("document.getElementById('btn-open-folder').getBoundingClientRect().height>0"), 'expanded secondary action');
  check('automatic detection starts once without opening or extracting a game', await value("calls.filter(c=>c.cmd==='scan_for_game').length===1 && !calls.some(c=>c.cmd==='open_game') && document.getElementById('game-scan-state').dataset.phase==='done' && !document.getElementById('btn-open-image').disabled"), 'read-only discovery at startup');
  await click('#game-alternatives summary');
  const brand = await value(`(()=>{const logo=document.querySelector('.brand .logo'), sub=document.querySelector('.brand-text'), a=logo.getBoundingClientRect(), b=sub.getBoundingClientRect();return {width:a.width,height:a.height,radius:getComputedStyle(logo).borderRadius,subtitleBelow:b.top>=a.bottom,centered:Math.abs((a.left+a.width/2)-(b.left+b.width/2))<1,noTitle:!document.querySelector('.brand strong'),loaded:logo.complete&&logo.naturalWidth>=256};})()`);
  check('brand matches MK Launcher 128px / 24px badge', brand.width===128 && brand.height===128 && brand.radius==='24px', brand);
  check('brand subtitle below centered logo without duplicate title', brand.subtitleBelow && brand.centered && brand.noTitle && brand.loaded, brand);
  await shot('00-brand-light-en');
  const footer = await value(`Array.from(document.querySelectorAll('.foot-row .foot-btn')).map(b=>({id:b.id,w:b.getBoundingClientRect().width,h:b.getBoundingClientRect().height,border:getComputedStyle(b).borderTopWidth,bg:getComputedStyle(b).backgroundColor,icon:b.querySelector('svg')?.getBoundingClientRect().width}))`);
  check('footer has Discord, Donate, MKSS tutorial, About and Settings', footer.map(b=>b.id).join(',')==='btn-discord,btn-donate,btn-tour,btn-about,btn-settings', footer);
  check('footer icons share size and have no button chrome', footer.every(b=>b.w===footer[0].w && b.h===footer[0].h && b.border==='0px' && b.bg==='rgba(0, 0, 0, 0)' && b.icon===17), footer);
  check('footer: icons and version on ONE line inside the sidebar; Donate tooltip is just "Donate"', await value(`(()=>{const v=document.getElementById('ver').getBoundingClientRect(),d=document.getElementById('btn-discord').getBoundingClientRect(),g=document.getElementById('btn-settings').getBoundingClientRect(),s=document.querySelector('.sidebar').getBoundingClientRect();return Math.abs((v.top+v.bottom)/2-(d.top+d.bottom)/2)<=1&&v.left>=g.right&&v.right<=s.right-8&&d.left>=s.left+8&&document.getElementById('btn-donate').title==='Donate'})()`), 'single footer row');
  const hoverStyles=[];
  for(const b of footer) {
    const [x,y]=await value(`(()=>{const r=document.getElementById('${b.id}').getBoundingClientRect();return [r.left+r.width/2,r.top+r.height/2];})()`);
    await send('Input.dispatchMouseEvent',{type:'mouseMoved',x,y,button:'none',buttons:0});
    hoverStyles.push(await value(`(()=>{const s=getComputedStyle(document.getElementById('${b.id}'));return [s.color,s.backgroundColor,s.borderTopWidth];})()`));
  }
  check('all five footer icons share hover behavior', hoverStyles.every(s=>JSON.stringify(s)===JSON.stringify(hoverStyles[0])) && hoverStyles[0][1]==='rgba(0, 0, 0, 0)' && hoverStyles[0][2]==='0px',hoverStyles);
  await click('#btn-discord');
  check('Discord opens the same Karps community through native opener', await value("calls.some(c=>c.cmd==='plugin:opener|open_url' && c.args.url==='https://discord.gg/93vfR7597T')"), 'external link, not embedded navigation');
  await click('#btn-donate');
  check('Donate opens exact Ko-fi URL through native opener',await value("calls.some(c=>c.cmd==='plugin:opener|open_url' && c.args.url==='https://ko-fi.com/m1t5')"),'external browser, no embedded navigation');
  check('Donate has localized accessible label and matches footer geometry',await value("(()=>{const b=document.getElementById('btn-donate'),r=b.getBoundingClientRect(),s=b.closest('.sidebar').getBoundingClientRect();return b.title.length>0&&b.textContent.trim().length>0&&r.left>=s.left&&r.right<=s.right})()"),'do not shrink hit targets or overflow sidebar');
  await click('#btn-about');
  check('About opens MKSS-specific popover', await value("!!document.querySelector('#about-pop:popover-open') && document.getElementById('about-pop').textContent.includes('MK Skin Studio') && document.getElementById('about-pop').textContent.includes('GPL-3.0') && !document.getElementById('about-pop').textContent.includes('Karps Engine')"), 'MKSS identity and license');
  await key('Escape', 'Escape', 27);
  check('About dismisses with Escape', await value("!document.querySelector('#about-pop:popover-open')"), 'native popover');
  await click('#btn-tour');
  check('MKSS tutorial opens without a game', await value("!!document.getElementById('tour-dialog')?.open && document.getElementById('tour-title').textContent.includes('MK Skin Studio')"), 'read-only guide');
  const tutorialPages=[];
  for(let i=0;i<5;i++) {
    tutorialPages.push(await value("document.getElementById('tour-text').textContent"));
    check(`tutorial step ${i+1} shown`, await value(`document.getElementById('tour-progress').textContent==='Step ${i+1} of 5' && document.getElementById('tour-back').disabled===${i===0}`), 'step count');
    if(i===2) { await click('#tour-back'); check('tutorial back navigation', await value("document.getElementById('tour-progress').textContent==='Step 2 of 5'"), 'previous page'); await click('#tour-next'); }
    if(i===4) await shot('00-tutorial-export-en');
    await click('#tour-next');
  }
  check('tutorial is specific to the MKSS project workflow', ['.mkss','ISO/WBFS','automatic detection','3D','project','Export mod (.zip)','Done','Save (Ctrl+S)'].every(w=>tutorialPages.join(' ').includes(w)), tutorialPages);
  check('tutorial finish preserves the current screen', await value("!document.getElementById('tour-dialog').open && window.__mkss.state.tab==='new' && !window.__mkss.state.game && !window.__mkss.state.project && document.activeElement.id==='btn-tour'"), 'guide does not edit or load game');
  check('settings controls are hidden until opened', await value("!!document.getElementById('btn-settings') && !!document.querySelector('dialog#settings-dialog') && !document.getElementById('settings-dialog').open && document.querySelector('.side-foot #opt-theme')===null"), 'compact settings footer');
  await click('#btn-settings');
  check('settings opens a modal', await value("document.getElementById('settings-dialog').open && document.activeElement.closest('#settings-dialog')!==null"), 'focus inside settings');
  await value("(()=>{const s=document.getElementById('opt-theme');s.value='dark';s.dispatchEvent(new Event('change',{bubbles:true}));})()");
  check('settings changes and persists theme', await value("document.documentElement.dataset.theme==='dark' && localStorage.getItem('karps.theme')==='dark'"), 'dark saved');
  await click('#lang-picker .lang-btn');
  await click('#lang-picker [data-l=pt]');
  check('settings changes and persists language', await value("document.documentElement.lang==='pt-BR' && localStorage.getItem('karps.lang')==='pt'"), 'pt saved');
  await shot('00-settings-dark-pt');
  const sidebarColors = await value("({bg:getComputedStyle(document.querySelector('.sidebar')).backgroundColor,sub:getComputedStyle(document.querySelector('.brand-text span')).color})");
  check('dark sidebar matches MK Launcher colors', sidebarColors.bg==='rgb(17, 17, 19)' && sidebarColors.sub==='rgb(133, 133, 142)', sidebarColors);
  await click('#btn-settings-close');
  check('settings close restores focus', await value("!document.getElementById('settings-dialog').open && document.activeElement.id==='btn-settings'"), 'focus returns to gear');
  await boot('light', 'en', 1280, 720);
  await toEditor();
  const T = "functional";
  check(`${T} 3D WebGL context`, await value(`${E}.preview.ok && !!${E}.preview.renderer.getContext()`), "");
  check(`${T} model shown`, await value(`!!${E}.preview.current && ${E}.preview.current.submeshes.length===2`), "");
  check(`${T} not-affiliated visible in editor`, await value(legalVisible), "");
  check(`${T} sidebar collapsed`, await value("document.body.classList.contains('editing') && document.querySelector('.sidebar').getBoundingClientRect().width<70"), "");
  await click('#btn-settings');
  check('settings available in compact editor', await value("document.getElementById('settings-dialog').open && document.getElementById('opt-theme').getBoundingClientRect().height>0 && getComputedStyle(document.querySelector('#lang-picker .lang-btn span')).display!=='none'"), 'theme and language visible in editor');
  await key('Escape', 'Escape', 27);
  await wait("!document.getElementById('settings-dialog').open");
  check('Escape closes settings in editor', await value("document.activeElement.id==='btn-settings'"), 'focus restored');
  check(`${T} paint on model is the default mode`, await value(`${E}.mode==='3d' && document.getElementById('mode-3d').getAttribute('aria-selected')==='true' && !!document.querySelector('#ed-stage > .p3d')`), "");
  await shot("10-editor-light-en");
  await toAdvanced();
  check(`${T} Advanced mode moves the 3D view to the side panel`, await value(`!!document.querySelector('.ed-right > .p3d') && !document.getElementById('tex-switch').closest('.hidden') && getComputedStyle(document.getElementById('tex-switch')).display!=='none'`), "");
  const before3d = await value(`${E}.preview.readCenter()`);

  await click("#parts-list li[data-i='0']");
  check(`${T} part selection mask`, await value(`!!${E}.current.selection && ${E}.current.selection[10]===255 && ${E}.current.selection[100]===0`), "");
  check(`${T} part highlighted`, await value("document.querySelector(\"#parts-list li[data-i='0']\").classList.contains('on')"), "");
  await value(`${E}.color.set([255,0,0,255]);${E}.setTool('brush');${E}.opts.brush.size=6;${E}.opts.brush.hardness=1`);
  const uploads0 = await value(`${E}.preview.uploads`);
  const a = await texel(20, 30), b = await texel(110, 30);
  const pts = [];
  for (let i = 0; i <= 12; i++) pts.push([a[0] + ((b[0] - a[0]) * i) / 12, a[1]]);
  await drag(pts);
  await sleep(150);
  const inside = await px(30, 30), outside = await px(100, 30);
  check(`${T} stroke painted inside the part`, inside.join() === "255,0,0,255", inside);
  check(`${T} stroke clipped outside the part`, outside.join() !== "255,0,0,255", outside);
  check(`${T} paint went to the active layer, base untouched`, await value(`(()=>{const d=${E}.current;const L=d.layers.find(l=>l.id===d.active);return !L.base && L.pixels[(30*d.w+30)*4+3]===255 && d.layers[0].pixels[(30*d.w+30)*4]===d.original[(30*d.w+30)*4];})()`), "");
  check(`${T} 3D texture re-uploaded`, (await value(`${E}.preview.uploads`)) > uploads0, "");
  const after3d = await value(`${E}.preview.readCenter()`);
  check(`${T} 3D pixel readback changed after painting`, after3d.length === 3 && Math.abs(after3d[0] - before3d[0]) + Math.abs(after3d[1] - before3d[1]) + Math.abs(after3d[2] - before3d[2]) > 3, { before3d, after3d });
  await shot("11-editor-painted-light-en");

  await value(`${E}.undo()`);
  await sleep(100);
  check(`${T} undo restores`, (await px(30, 30)).join() !== "255,0,0,255", await px(30, 30));
  await value(`${E}.redo()`);
  await sleep(100);
  check(`${T} redo re-applies`, (await px(30, 30)).join() === "255,0,0,255", await px(30, 30));
  await key("z", "KeyZ", 90, 2);
  check(`${T} Ctrl+Z`, (await px(30, 30)).join() !== "255,0,0,255", "");
  await key("y", "KeyY", 89, 2);
  check(`${T} Ctrl+Y`, (await px(30, 30)).join() === "255,0,0,255", "");

  await key("i", "KeyI", 73);
  check(`${T} shortcut I selects eyedropper`, (await value(`${E}.tool`)) === "picker", "");
  const p = await texel(90, 40);
  await drag([p]);
  const want = await px(90, 40);
  check(`${T} eyedropper`, (await value(`${E}.color.primary`)).join() === want.join(), { want, got: await value(`${E}.color.primary`) });

  await click("#parts-list li[data-i='1']");
  await click("#lay-add");
  await value(`${E}.color.set([0,0,255,255]);${E}.setTool('fill');${E}.opts.fillTolerance=255;${E}.opts.fillContiguous=true`);
  await drag([await texel(100, 10)]);
  await sleep(100);
  check(`${T} fill inside part`, (await px(70, 5)).join() === "0,0,255,255" && (await px(127, 63)).join() === "0,0,255,255", await px(70, 5));
  check(`${T} fill clipped to part`, (await px(30, 30)).join() === "255,0,0,255" && (await px(5, 5)).join() !== "0,0,255,255", await px(5, 5));

  check(`${T} layers added`, (await value(`${E}.current.layers.length`)) === 3, "");
  await click("#layer-list li.on .vis");
  check(`${T} hide layer`, (await px(70, 5)).join() !== "0,0,255,255", "");
  await click("#layer-list li.on .vis");
  const setOpacity = (v) => value(`(()=>{const s=document.getElementById('lay-opacity');s.value='${v}';s.dispatchEvent(new Event('input'));s.dispatchEvent(new Event('change'));})()`);
  await setOpacity(50);
  await sleep(100);
  const half = await px(70, 5);
  check(`${T} opacity 50% blends`, half.join() !== "0,0,255,255" && half[2] >= 127 && Math.max(half[0], half[1]) >= 60 && half[3] === 255, half);
  await setOpacity(100);
  const order0 = await value(`${E}.current.layers.map(l=>l.id).join()`);
  await value(`(()=>{const list=document.getElementById('layer-list');const top=list.querySelector('li.on');const target=[...list.querySelectorAll('li')].find(li=>li.dataset.id==='2');const dt=new DataTransfer();top.dispatchEvent(new DragEvent('dragstart',{dataTransfer:dt,bubbles:true}));target.dispatchEvent(new DragEvent('dragover',{dataTransfer:dt,bubbles:true,cancelable:true}));target.dispatchEvent(new DragEvent('drop',{dataTransfer:dt,bubbles:true,cancelable:true}));})()`);
  await sleep(100);
  const order1 = await value(`${E}.current.layers.map(l=>l.id).join()`);
  check(`${T} reorder layers by drag`, order0 !== order1 && order1.startsWith("1,"), { order0, order1 });
  await value(`(()=>{const s=document.getElementById('lay-blend');s.value='multiply';s.dispatchEvent(new Event('change'));})()`);
  await sleep(80);
  check(`${T} blend mode changes composite`, (await px(100, 50)).join() !== "0,0,255,255", await px(100, 50));
  await value(`${E}.undo()`);
  await value(`${E}.undo()`);
  await sleep(80);
  check(`${T} undo layer ops`, (await value(`${E}.current.layers.map(l=>l.id).join()`)) === order0 && (await px(70, 5)).join() === "0,0,255,255", await value(`${E}.current.layers.map(l=>l.id).join()`));

  await click("#parts-list li[data-i='0']");
  const beforeRc = await px(10, 50);
  await click("#lay-recolor");
  check(`${T} adjust panel`, await value("!document.getElementById('adjust-panel').classList.contains('hidden')"), "");
  await value("(()=>{const s=document.getElementById('sl-hue');s.value='120';s.dispatchEvent(new Event('input'));s.dispatchEvent(new Event('change'));})()");
  await sleep(100);
  const afterRc = await px(10, 50), otherRc = await px(100, 50);
  check(`${T} recolor adjusts the part`, afterRc.join() !== beforeRc.join(), { beforeRc, afterRc });
  check(`${T} recolor leaves the other part`, otherRc.join() === "0,0,255,255", otherRc);
  await shot("12-editor-layers-light-en");

  await click("#parts-clear");
  await value(`${E}.replaceFrom=[0,0,255,255];${E}.color.set([0,255,0,255])`);
  await click("#btn-replace-color");
  await click("#rc-apply");
  await sleep(100);
  check(`${T} replace colour`, (await px(100, 50)).join() === "0,255,0,255", await px(100, 50));

  const n0 = await value(`${E}.current.layers.length`);
  await click("#lay-import");
  await wait(`${E}.current.layers.length===${n0 + 1}`);
  check(`${T} import image as layer`, (await value(`(()=>{const d=${E}.current;return d.layers.find(l=>l.id===d.active).source?.name})()`)) === "green.png", "");
  check(`${T} imported pixels`, (await value(`(()=>{const d=${E}.current;const L=d.layers.find(l=>l.id===d.active);return Array.from(L.pixels.slice(0,4)).join()})()`)) === "0,255,0,255", "");
  await click("#lay-del");

  await value(`${E}.setSelection(null);${E}.preview.resetView()`);
  await sleep(150);
  const sp = await value(`${E}.preview.screenOfPart('mario_all',1)`);
  check(`${T} part visible in 3D`, Array.isArray(sp), sp);
  await mouse("mouseMoved", sp[0], sp[1], "none");
  await mouse("mousePressed", sp[0], sp[1]);
  await mouse("mouseReleased", sp[0], sp[1]);
  await sleep(150);
  check(`${T} click in 3D selects the part`, await value(`!!${E}.focus && ${E}.focus.index===1 && document.querySelector("#parts-list li[data-i='1']").classList.contains('on') && ${E}.current.selection[100]===255`), await value(`${E}.focus`));
  await shot("13-editor-3d-pick-light-en");

  await click("#tex-switch");
  check(`${T} texture switcher grid`, (await value("document.querySelectorAll('#tex-pop li[data-name]').length")) === 6, "");
  await click("#tex-pop li[data-name='mario_eye.2']");
  check(`${T} eye warning`, await value("!document.getElementById('eye-warn').classList.contains('hidden')"), "");
  check(`${T} eye frame uses frame 0 UVs`, await value(`${E}.texParts('mario_eye.2').aliasOf==='mario_eye.0'`), "");
  await value(`${E}.setTool('brush');${E}.color.set([255,255,0,255])`);
  const e1 = await texel(16, 30);
  await drag([e1, [e1[0] + 5, e1[1] + 5]]);
  await click("#btn-eyes-all");
  check(`${T} apply to all eye frames`, await value(`['mario_eye.0','mario_eye.1','mario_eye.3'].every(n=>${E}.edited(n))`), "");
  await click("#tex-switch");
  await click("#tex-pop li[data-name='mario_all']");

  await value(`(()=>{const d=${E}.current;d.active=d.layers[0].id;${E}.renderLayers();})()`);
  await click("#lay-dup");
  const saved = await hashComposite();
  const savedEye = await hashComposite("mario_eye.3");
  check(`${T} unsaved changes shown in the Studio header`, await value("document.getElementById('save-state').textContent==='Unsaved changes' && window.__mkss.dirty()"), "");
  await click("#btn-proj-save");
  await wait("calls.some(c=>c.cmd==='project_char_write')");
  await sleep(100);
  const proj = await value(`(()=>{const p=pstore[${S}.project.id].data.mr;return p?{m:JSON.parse(p.manifest),n:p.files.length}:null})()`);
  check(`${T} character saved into the project`, !!proj && proj.m.format === "mkss-project/1.1" && proj.m.character.code === "mr", proj?.m?.format);
  check(`${T} Save marks the Studio as saved`, await value("document.getElementById('save-state').textContent==='Saved' && !window.__mkss.dirty()"), "");
  const allTex = proj.m.textures.find((x) => x.name === "mario_all");
  const dup = allTex.layers.find((l) => !l.base && l.keep);
  check(`${T} base duplicate stored as keep-mask only (no original texels)`, !!dup && !dup.data, allTex.layers.map((l) => [l.name, !!l.data, !!l.keep]));
  const baseL = allTex.layers.find((l) => l.base);
  check(`${T} untouched base stored without pixel data`, !!baseL && !baseL.data, baseL);
  check(`${T} unchanged textures not in project`, !proj.m.textures.some((x) => x.name === "mario_tx_64"), "");
  check(`${T} edit tags recorded for the Characters page`, await value(`(()=>{const c=pstore[${S}.project.id].characters.find(c=>c.code==='mr');return c.edits.includes('body') && c.edits.includes('eyes');})()`), await value(`pstore[${S}.project.id].characters`));
  await click("#btn-done");
  await tab('characters');
  await openChar('fk');
  check(`${T} another character starts from the original`, await value(`${E}.editedDocs().length===0`), "");
  await click("#btn-done");
  await tab('characters');
  await openChar('mr');
  await sleep(300);
  check(`${T} project reopened identical`, (await hashComposite("mario_all")) === saved && (await hashComposite("mario_eye.3")) === savedEye && (await value("!window.__mkss.dirty()")), { saved });

  await value(`${E}.color.set([10,20,30,255]);${E}.setTool('brush')`);
  await toAdvanced();
  await drag([await texel(3, 3)]);
  await value("window.__mkss.autosave()");
  check(`${T} autosave written per project/character`, await value(`(()=>{const a=store['auto:'+${S}.project.id+':mr'];return !!a && JSON.parse(a.manifest).textures.length>0;})()`), "");
  const autoHash = await hashComposite("mario_all");

  await boot("dark", "en", 1280, 720, "empty", true);
  await value("window.offerAutosave=true");
  check(`${T} project library survives a restart`, await value("document.querySelectorAll('#proj-list .proj-card').length===1"), "");
  await click("#proj-list .proj-card [data-act=open]");
  await tab('characters');
  await openChar('mr');
  await wait("!document.getElementById('recover-banner').classList.contains('hidden')");
  check(`${T} recovery banner names the character`, await value("document.getElementById('recover-text').textContent.includes('Mario')"), "");
  await click("#btn-recover");
  await sleep(500);
  check(`${T} autosave recovery restores work (not yet saved)`, (await hashComposite("mario_all")) === autoHash && (await value("window.__mkss.dirty() && document.getElementById('save-state').textContent==='Unsaved changes'")), "");
  await leaveTo('characters', 'discard');
  check(`${T} discarding clears the recovery file`, await value(`!store['auto:'+${S}.project.id+':mr']`), "");

  await boot("dark", "en", 1280, 720);
  await value("window.failModel=true");
  await newProject();
  await click("#char-grid button.char[data-code=mr]");
  await wait(`!!${E}.current && !!${E}.modelError`);
  check(`${T} model failure falls back to Advanced (texture)`, await value(`${E}.mode==='2d' && document.getElementById('mode-3d').disabled`), "");
  check(`${T} model failure message`, await value("!document.getElementById('p3d-msg').classList.contains('hidden') && document.getElementById('p3d-msg').textContent.length>10"), "");
  await value(`${E}.setTool('brush');${E}.color.set([255,0,255,255]);${E}.opts.brush.size=3`);
  await drag([await texel(5, 5)]);
  check(`${T} 2D works without a model`, (await px(5, 5)).join() === "255,0,255,255", await px(5, 5));
  await shot("15-editor-nomodel-dark-en");
}

async function portraitScenarios() {
  await boot('dark','en',1280,720);
  await newProject();
  await wait("document.querySelectorAll('#char-grid img').length===24 && [...document.querySelectorAll('#char-grid img')].every(i=>i.complete)");
  check('all character cards use decoded portrait instead of initials',await value("document.querySelectorAll('#char-grid .char-portrait img').length===24 && !document.querySelector('#char-grid .mono') && [...document.querySelectorAll('#char-grid img')].every(i=>i.naturalWidth===64 && i.naturalHeight===64 && i.alt==='')"),'24 loaded decorative images with visible names');
  check('portrait sources map to the exact character code',await value("[...document.querySelectorAll('#char-grid button')].every(b=>b.querySelector('img').src===window.__mkss.state.game.characters.find(c=>c.code===b.dataset.code).portrait)"),'backend mapping preserved');
  await shot('10-character-portraits-dark-en');
  await click('#char-grid [data-code=fk]');
  await wait("window.__mkss.state.tab==='studio'");
  check('portrait click selects same character without changing workflow',await value("calls.find(c=>c.cmd==='load_character').args.code==='fk' && window.__mkss.state.character.code==='fk'"),'Funky Kong selected');
  for (const flag of ['missingPortraits','brokenPortraits']) {
    await boot('light','pt',1280,720);
    await value(`window.${flag}=true`);
    await newProject();
    await wait("document.querySelectorAll('#char-grid .char-portrait svg').length===24");
    check(`${flag} has neutral fallback without broken image or initials`,await value("!document.querySelector('#char-grid img, #char-grid .mono') && document.querySelectorAll('#char-grid button').length===24 && [...document.querySelectorAll('#char-grid .char-name')].every(e=>e.textContent.length>0)"),'incomplete extracted folder remains usable');
    check(`${flag} cards fit compact window`,(await value(layoutProbe)).length===0,'no clipping');
  }
}

async function discoveryScenarios() {
  await value("localStorage.removeItem('mkss.lastGamePath')");
  await boot('dark','en',1280,720,'found');
  await toNew();
  await wait("document.getElementById('game-scan-state').dataset.phase==='done'");
  check('confirmation is the only primary action in game card',await value("document.querySelectorAll('.game-open-card button.primary').length===1 && document.querySelector('.game-open-card button.primary').id==='btn-use-detected' && getComputedStyle(document.getElementById('btn-open-image')).borderTopWidth==='0px' && getComputedStyle(document.getElementById('btn-scan-game')).backgroundColor==='rgba(0, 0, 0, 0)' && document.getElementById('btn-open-image').textContent==='Change…'"),'detection first, discreet change');
  check('secondary game actions are cyan and underlined without hover',await value("[...document.querySelectorAll('.game-text-action')].every(e=>{const s=getComputedStyle(e);const probe=document.createElement('span');probe.style.color='var(--accent-text)';e.append(probe);const c=getComputedStyle(probe).color;probe.remove();return s.color===c && s.textDecorationLine.includes('underline') && s.backgroundColor==='rgba(0, 0, 0, 0)';})"),'recognizable text links');
  check('detection lists supported files without opening them',await value("document.querySelectorAll('#detected-game option').length===2 && !calls.some(c=>c.cmd==='open_game') && window.__mkss.state.tab==='new'"),'requires explicit confirmation');
  await value("(()=>{const s=document.getElementById('detected-game');s.value='E:/Wii/Mario Kart Wii (Europe).iso';s.dispatchEvent(new Event('change',{bubbles:true}));})()");
  check('detected selection shows exact selected path',await value("document.getElementById('detected-path').textContent==='E:/Wii/Mario Kart Wii (Europe).iso'"),'multiple copies');
  await shot('07-detected-dark-en');
  await click('#btn-use-detected');
  await wait("window.__mkss.state.tab==='characters'");
  check('detected copy opens only on explicit action and remembers successful path',await value("calls.filter(c=>c.cmd==='open_game').length===1 && calls.find(c=>c.cmd==='open_game').args.path==='E:/Wii/Mario Kart Wii (Europe).iso' && localStorage.getItem('mkss.lastGamePath')==='E:/Wii/Mario Kart Wii (Europe).iso'"),'selected copy preserved');
  check('new project created with the detected game and its disc ID', await value("calls.find(c=>c.cmd==='project_create').args.gamePath==='E:/Wii/Mario Kart Wii (Europe).iso' && calls.find(c=>c.cmd==='project_create').args.gameId==='RMCE01'"), 'game remembered in the project');
  await toNew();
  check('next new project is one click: remembered game preselected and name prefilled', await value("document.getElementById('detected-game').value==='E:/Wii/Mario Kart Wii (Europe).iso' && !document.getElementById('btn-use-detected').disabled && document.getElementById('new-name').value==='My skin pack 2'"), 'one click');
  await click('#btn-scan-game');
  check('rescan prioritizes remembered game hint',await value("calls.filter(c=>c.cmd==='scan_for_game').at(-1).args.lastPath==='E:/Wii/Mario Kart Wii (Europe).iso' && document.getElementById('detected-game').value==='E:/Wii/Mario Kart Wii (Europe).iso'"),'native still validates hint');

  await boot('dark','en',1280,720,'deferred');
  await toNew();
  check('background scan leaves manual ISO/WBFS selection available',await value("document.getElementById('game-scan-state').dataset.phase==='scanning' && document.getElementById('btn-scan-game').disabled && !document.getElementById('btn-open-image').disabled && document.getElementById('busy').classList.contains('hidden')"),'non-blocking');
  await click('#btn-scan-game');
  check('duplicate scan clicks do not start parallel searches',await value("calls.filter(c=>c.cmd==='scan_for_game').length===1"),'in-flight scan coalesced');
  await click('#btn-open-image');
  check('manual choice waits for explicit confirmation',await value("window.__mkss.state.tab==='new' && !calls.some(c=>c.cmd==='open_game') && document.getElementById('detected-path').textContent==='D:/Games/Manual game.wbfs' && !document.getElementById('btn-use-detected').disabled"),'picker is selection, not navigation');
  await value("scanWaiters.shift()(discoveredFixture())");
  check('late scan preserves manually selected unconfirmed game',await value("document.getElementById('detected-path').textContent==='D:/Games/Manual game.wbfs' && !calls.some(c=>c.cmd==='open_game')"),'manual selection wins');
  await click('#btn-use-detected');
  await wait("window.__mkss.state.tab==='characters'");
  check('late detection never overrides a manually opened game',await value("window.__mkss.state.game.root==='D:/Games/Manual game.wbfs' && window.__mkss.state.tab==='characters' && localStorage.getItem('mkss.lastGamePath')==='D:/Games/Manual game.wbfs'"),'stale scan discarded');

  await boot('light','en',1280,720,'error');
  await toNew();
  await wait("document.getElementById('game-scan-state').dataset.phase==='error'");
  check('scan failure remains actionable and is not called game-not-found',await value("document.getElementById('game-scan-state').textContent.includes('Could not') && !document.getElementById('btn-open-image').disabled && !document.getElementById('btn-scan-game').disabled"),'honest failure state');
  await value("window.scanMode='found'");
  await click('#btn-scan-game');
  check('retry recovers after discovery failure',await value("document.querySelectorAll('#detected-game option').length===2 && document.getElementById('game-scan-state').dataset.phase==='done'"),'retry works');
  await value("window.failOpen=true;localStorage.setItem('mkss.lastGamePath','D:/previous-valid.wbfs')");
  await click('#btn-use-detected');
  check('failed game validation does not replace remembered successful path',await value("localStorage.getItem('mkss.lastGamePath')==='D:/previous-valid.wbfs' && window.__mkss.state.tab==='new' && !document.getElementById('btn-use-detected').disabled"),'retry selection preserved');

  for (const sample of [{"raw": "\\\\?\\C:\\Games\\Mario Kart Wii.wbfs", "display": "C:\\Games\\Mario Kart Wii.wbfs"}, {"raw": "\\\\?\\UNC\\server\\games\\Mario Kart Wii.iso", "display": "\\\\server\\games\\Mario Kart Wii.iso"}]) {
    await boot('dark','en',1280,720,'deferred');
  await toNew();
    await value(`scanWaiters.shift()({games:[{path:${JSON.stringify(sample.raw)},id:'RMCE01',region:'USA',format:'WBFS',sizeBytes:2800000000}],truncated:true})`);
    await wait("document.getElementById('game-scan-state').dataset.phase==='done'");
    check('extended Windows path is cleaned only for display',await value(`document.getElementById('detected-path').textContent===${JSON.stringify(sample.display)} && document.getElementById('detected-game').value===${JSON.stringify(sample.raw)}`),'native path preserved');
    await click('#btn-use-detected');
    await wait("window.__mkss.state.tab==='characters'");
    check('opening preserves exact native extended path',await value(`calls.find(c=>c.cmd==='open_game').args.path===${JSON.stringify(sample.raw)} && localStorage.getItem('mkss.lastGamePath')===${JSON.stringify(sample.raw)}`),'no path mutation');
  }

  await boot('dark','en',1280,720,'single');
  await toNew();
  await wait("document.getElementById('game-scan-state').dataset.phase==='done'");
  check('single detection displays filename without unnecessary dropdown',await value("document.getElementById('detected-choice').classList.contains('hidden') && !document.getElementById('game-selected-name').classList.contains('hidden') && document.getElementById('game-selected-name').textContent==='Mario Kart Wii (USA).wbfs' && document.getElementById('btn-open-image').textContent==='Change…'"),'one game, one confirmation');
  check('confirmation sits just above Other options divider',await value("(()=>{const b=document.getElementById('btn-use-detected').getBoundingClientRect(),d=document.getElementById('game-alternatives').getBoundingClientRect(),s=document.querySelector('.game-discovery').getBoundingClientRect();return d.top-b.bottom>=12 && d.top-b.bottom<=18 && Math.abs(b.right-s.right)<1})()"),'bottom-right CTA, 16px divider gap');
  await shot('09-single-detected-dark-en');
  await value("window.cancelPicker=true");
  await click('#btn-open-image');
  check('cancel change preserves detected selection without opening',await value("document.getElementById('detected-path').textContent==='D:/Games/Mario Kart Wii (USA).wbfs' && !calls.some(c=>c.cmd==='open_game') && !document.getElementById('btn-use-detected').disabled"),'cancel safe');
  await boot('light','en',1280,720,'deferred');
  await toNew();
  await value("scanWaiters.shift()({games:[],truncated:true})");
  await wait("document.getElementById('game-scan-state').dataset.phase==='done'");
  check('incomplete empty search retains useful fallback disclosure',await value("!document.getElementById('game-scan-limit').classList.contains('hidden') && !document.getElementById('btn-open-image').disabled && document.getElementById('btn-use-detected').disabled"),'no hidden reason when no game was found');
  await boot('light','en',1280,720,'empty');
  await toNew();
  await wait("document.getElementById('game-scan-state').dataset.phase==='done'");
  check('no detection offers subtle manual fallback and disabled confirmation',await value("document.getElementById('btn-open-image').textContent==='Choose manually…' && document.getElementById('btn-use-detected').disabled && document.getElementById('game-detected').classList.contains('hidden')"),'no false next step');
  await click('#btn-open-image');
  check('manual fallback stages file and focuses confirmation',await value("!calls.some(c=>c.cmd==='open_game') && document.activeElement.id==='btn-use-detected' && !document.getElementById('btn-use-detected').disabled && document.getElementById('game-scan-state').textContent.includes('File selected')"),'manual and auto share confirmation');
  await click('#btn-use-detected');
  await wait("window.__mkss.state.tab==='characters'");
  check('manual fallback advances only after confirmation',await value("calls.filter(c=>c.cmd==='open_game').length===1 && window.__mkss.state.game.root==='D:/Games/Manual game.wbfs'"),'single open');

  for(const lang of ['en','pt','es','fr','ja']) {
    await boot('dark',lang,1280,720,'limited');
    await toNew();
    await wait("document.getElementById('game-scan-state').dataset.phase==='done'");
    check(`${lang} detected game has concise format note without search-limit noise`,await value("document.getElementById('game-scan-limit').classList.contains('hidden') && document.querySelector('[data-i18n=\"game.imageText\"]').textContent.includes('.iso') && document.querySelector('[data-i18n=\"game.imageText\"]').textContent.includes('.wbfs') && document.querySelector('[data-i18n=\"game.imageText\"]').textContent.length<55"),'only supported formats');
    await click('#game-alternatives summary');
    const bounds=await value(layoutProbe);
    check(`${lang} detected plus expanded alternatives fit compact window`,bounds.length===0,bounds);
  }
}

async function layout(theme, lang, w, h, capture) {
  await boot(theme, lang, w, h);
  const tag = `${theme}-${lang}-${w}x${h}`;
  const screens = {};
  const panelScrollProbe = `(()=>[...document.querySelectorAll('.view:not(.hidden), .view:not(.hidden) .panel, .view:not(.hidden) .project-feedback')].filter(e=>e.getClientRects().length && (e.scrollHeight>e.clientHeight+1 || e.scrollWidth>e.clientWidth+1)).map(e=>({id:e.id,cls:e.className,client:[e.clientWidth,e.clientHeight],scroll:[e.scrollWidth,e.scrollHeight]})))()`;
  check(`${tag} theme applied`, (await value("document.documentElement.dataset.theme")) === theme, theme);
  check(`${tag} not-affiliated visible (start)`, await value(legalVisible), "");
  check(`${tag} projects home: empty state with one primary action`, await value("!document.getElementById('proj-empty').classList.contains('hidden') && [...document.querySelectorAll('#view-projects button.primary')].filter(b=>b.getClientRects().length).length===1"), "");
  screens.projectsEmpty = await value(layoutProbe);
  if (capture) await shot(`01-projects-empty-${theme}-${lang}`);
  await click('#btn-about');
  screens.about = await value(layoutProbe);
  if(capture && theme==='dark' && lang==='en') await shot('06-about-dark-en');
  await key('Escape','Escape',27);
  await click('#btn-settings');
  screens.settings = await value(layoutProbe);
  await click('#btn-settings-close');
  await click('#btn-tour');
  for(let page=0;page<5;page++) {
    screens[`tutorial${page+1}`] = await value(layoutProbe);
    await click('#tour-next');
  }
  await toNew();
  if (!(await value("document.getElementById('game-alternatives').open"))) await click("#game-alternatives summary");
  screens.newProject = await value(layoutProbe);
  { const overflow=await value(panelScrollProbe);check(`${tag} newProject structural panels need no scrollbar`,overflow.length===0,overflow); }
  if (capture) await shot(`02-new-project-${theme}-${lang}`);
  await click("#btn-open-folder");
  await tab('characters');
  screens.characters = await value(layoutProbe);
  check(`${tag} all 24 characters fit without scrolling`, await value("(()=>{const g=document.getElementById('char-grid'),r=g.getBoundingClientRect();return g.scrollHeight<=g.clientHeight+1 && g.scrollWidth<=g.clientWidth+1 && [...g.children].length===24 && [...g.children].every(c=>c.getBoundingClientRect().bottom<=r.bottom+.5)})()"), "no hidden final row");
  await openChar('pc');
  check(`${tag} not-affiliated visible (editor)`, await value(legalVisible), "");
  check(`${tag} outfit bar visible at the top`, await value(`(()=>{const o=document.getElementById('ed-outfit').getBoundingClientRect(),t=document.querySelector('.ed-top').getBoundingClientRect();return o.height>0&&Math.abs(o.top-t.bottom)<1.5&&!document.getElementById('vehicle-group').classList.contains('hidden')&&!document.getElementById('detail-group').classList.contains('hidden');})()`), "");
  check(`${tag} Studio header: breadcrumb and Done visible, no export`, await value(`(()=>{const c=document.getElementById('crumb-char').getBoundingClientRect(),d=document.getElementById('btn-done').getBoundingClientRect();return c.width>0&&d.width>0&&d.right<=innerWidth&&!document.getElementById('btn-build')&&!document.getElementById('btn-to-export')&&document.getElementById('crumb-char').textContent==='Peach';})()`), "");
  screens.editor = await value(layoutProbe);
  { const overflow=await value(panelScrollProbe);check(`${tag} editor structural panels need no scrollbar`,overflow.length===0,overflow); }
  await click("#veh-bike");
  await click("#det-low");
  screens.outfitLow = await value(layoutProbe);
  if (capture) await shot(`03-outfit-bike-low-${theme}-${lang}`);
  await value(`${E}.setTool('hand')`);
  screens.hand = await value(layoutProbe);
  await value(`${E}.setTool('brush')`);
  await click("#det-full");
  await click("#veh-kart");
  check(`${tag} eye panel shown in paint mode`, await value("!document.getElementById('eye-panel').classList.contains('hidden')"), "");
  if (capture) await shot(`03-paint3d-${theme}-${lang}`);
  await click("#parts-toggle");
  screens.partsCollapsed = await value(layoutProbe);
  await click("#parts-toggle");
  await toAdvanced();
  screens.advanced = await value(layoutProbe);
  { const overflow=await value(panelScrollProbe);check(`${tag} advanced structural panels need no scrollbar`,overflow.length===0,overflow); }
  await click("#parts-list li[data-i='0']");
  await click("#lay-recolor");
  screens.editorAdjust = await value(layoutProbe);
  { const overflow=await value(panelScrollProbe);check(`${tag} editorAdjust structural panels need no scrollbar`,overflow.length===0,overflow); }
  if (capture) await shot(`03-editor-${theme}-${lang}`);
  await click("#tex-switch");
  screens.switcher = await value(layoutProbe);
  if (capture) await shot(`04-switcher-${theme}-${lang}`);
  await click("#tex-pop li[data-name='mario_eye.1']");
  screens.eye = await value(layoutProbe);
  await click("#btn-help");
  screens.help = await value(layoutProbe);
  await value("document.getElementById('help-pop').classList.add('hidden')");
  await click("#btn-replace-color");
  screens.replace = await value(layoutProbe);
  await click("#rc-cancel");
  await value(`${E}.setTool('brush');${E}.color.set([200,30,30,255])`);
  await drag([await texel(10, 10), await texel(14, 12)]);
  await click("#btn-done");
  await wait("document.getElementById('ask-dialog').open");
  screens.unsavedDialog = await value(layoutProbe);
  if (capture) await shot(`05-unsaved-dialog-${theme}-${lang}`);
  await answer('save');
  await tab('characters');
  check(`${tag} saved character shows the Edited badge`, await value("document.querySelector('.char-cell.edited [data-code=pc]')!==null && document.querySelector('.char-cell.edited .b.ed').textContent.length>2"), "");
  check(`${tag} not-affiliated visible (characters)`, await value(legalVisible), "");
  await click('#btn-export-mod');
  await wait("!document.getElementById('export-done').classList.contains('hidden')");
  screens.charactersEdited = await value(layoutProbe);
  if (capture) await shot(`05-characters-edited-${theme}-${lang}`);
  await click(".app-tabs [data-tab=projects]");
  await tab('projects');
  await click("#proj-list .proj-card [data-act=menu]");
  screens.projectsFilled = await value(layoutProbe);
  if (capture) await shot(`06-projects-${theme}-${lang}`);
  for (const [k, v] of Object.entries(screens)) check(`${tag} ${k} no scroll/clipping`, v.length === 0, v.slice(0, 5));
}

async function paint3d() {
  await boot("light", "en", 1280, 720);
  await toEditor();
  const T = "paint3d";
  await value(`window.__base=(n)=>{window.__b=window.__b||{};window.__b[n]=Array.from(${E}.pixelsOf(${E}.docs.get(n)));}`);
  await value(`window.__diff=(n,rects=[])=>{const d=${E}.docs.get(n);const c=${E}.pixelsOf(d);const b=window.__b[n];let k=0,x0=1e9,y0=1e9,x1=-1,y1=-1;const inR=rects.map(()=>0);for(let i=0;i<d.w*d.h;i++){if(c[i*4]!==b[i*4]||c[i*4+1]!==b[i*4+1]||c[i*4+2]!==b[i*4+2]||c[i*4+3]!==b[i*4+3]){k++;const x=i%d.w,y=(i/d.w)|0;x0=Math.min(x0,x);y0=Math.min(y0,y);x1=Math.max(x1,x);y1=Math.max(y1,y);rects.forEach((r,j)=>{if(x>=r[0]&&x<r[2]&&y>=r[1]&&y<r[3])inR[j]++;});}}return {k,box:[x0,y0,x1,y1],inR};}`);
  const names = ["mario_all", "mario_eye.0", "mario_eye.1", "mario_eye.2", "mario_eye.3", "mario_tx_64"];
  const base = () => value(`${JSON.stringify(names)}.forEach(n=>window.__base(n))`);
  const scr = (p) => value(`${E}.preview.screenOf(${JSON.stringify(p)})`);
  const line = async (a, b, n = 10) => {
    const A = await scr(a), B = await scr(b);
    const pts = [];
    for (let i = 0; i <= n; i++) pts.push([A[0] + ((B[0] - A[0]) * i) / n, A[1] + ((B[1] - A[1]) * i) / n]);
    return pts;
  };
  check(`${T} default mode is paint on model`, await value(`${E}.mode==='3d' && ${E}.preview.paintMode && !!document.querySelector('#ed-stage > .p3d')`), "");
  check(`${T} 2D-only tools hidden on the model`, await value(`['select','move','line','rect'].every(t=>document.querySelector('#ed-tools [data-tool='+t+']').classList.contains('hidden')) && ['brush','eraser','fill','picker','wand'].every(t=>!document.querySelector('#ed-tools [data-tool='+t+']').classList.contains('hidden'))`), "");
  check(`${T} technical texture list hidden by default`, await value(`getComputedStyle(document.getElementById('tex-switch')).display==='none'`), "");
  check(`${T} eye controls shown (character has eye frames)`, await value(`!document.getElementById('eye-panel').classList.contains('hidden') && document.querySelectorAll('#eye-frame option').length===5`), "");
  await value(`${E}.resetView?.();${E}.preview.resetView();${E}.color.set([255,0,0,255]);${E}.setTool('brush');${E}.opts.size3d.brush=24;${E}.opts.brush.hardness=1;${E}.opts.brush.opacity=1`);
  await sleep(150);

  await base();
  const u0 = await value(`${E}.preview.uploads`);
  const sA = await line([-45, 40, 0], [-20, 40, 0]);
  const hov = await scr([-45, 40, 0]);
  await mouse("mouseMoved", hov[0], hov[1], "none");
  await sleep(60);
  check(`${T} brush cursor ring on the surface`, await value(`${E}.preview.ringVisible`), await value(`(()=>{const h=${E}.preview.hitAt(${hov[0]},${hov[1]});return {hov:${JSON.stringify(hov)},hit:h&&{t:h.texture,uv:h.uv,d:h.distance},mode:${E}.mode,subs:${E}.preview.subMeshes.length,rect:JSON.stringify(${E}.preview.renderer.domElement.getBoundingClientRect())};})()`));
  await drag(sA);
  await sleep(150);
  let dA = await value(`window.__diff('mario_all',[[0,0,64,64],[64,0,128,64]])`);
  check(`${T} 3D stroke changed body texels`, dA.k > 20, dA);
  check(`${T} 3D stroke stays in the UV of the part under the cursor`, dA.inR[0] === dA.k && dA.inR[1] === 0, dA);
  check(`${T} 3D stroke landed on the active (paint) layer, base untouched`, await value(`(()=>{const d=${E}.docs.get('mario_all');const L=d.layers.find(l=>l.id===d.active);return !L.base && d.layers[0].pixels.every((v,i)=>v===d.original[i]);})()`), "");
  check(`${T} eye frames untouched by a body stroke`, (await value(`[0,1,2,3].map(i=>window.__diff('mario_eye.'+i).k)`)).every((k) => k === 0), "");
  check(`${T} 3D view re-uploaded live`, (await value(`${E}.preview.uploads`)) > u0, "");
  const lowA = await value(`window.__diff('mario_tx_64',[[0,0,32,64],[32,0,64,64]])`);
  check(`${T} low-detail texture derived from the body edit (mapped half only)`, lowA.k > 0 && lowA.inR[1] === 0, lowA);
  check(`${T} low-detail derived marked edited`, await value(`${E}.edited('mario_tx_64') && ${E}.summary(${E}.docs.get('mario_tx_64')).derivedFrom==='mario_all'`), "");
  await shot("20-paint3d-body-light-en");

  await base();
  const sCD = await line([-25, -27, 0], [25, -27, 0], 16);
  await mouse("mouseMoved", sCD[0][0], sCD[0][1], "none");
  await mouse("mousePressed", sCD[0][0], sCD[0][1], "left");
  for (const p of sCD.slice(1)) await mouse("mouseMoved", p[0], p[1], "left");
  const mirrorDuring = await value(`({tris:${E}.preview.mirrorTriangles,hint:!document.getElementById('mirror-hint').classList.contains('hidden'),outline:${E}.preview.mirrorIsOutline})`);
  await shot("21-paint3d-seam-mirror-light-en");
  await mouse("mouseReleased", sCD[sCD.length - 1][0], sCD[sCD.length - 1][1], "left");
  await sleep(150);
  const dCD = await value(`window.__diff('mario_all',[[32,38,49,58],[111,38,128,58]])`);
  check(`${T} stroke across a UV seam paints both sides`, dCD.inR[0] > 10 && dCD.inR[1] > 10, dCD);
  check(`${T} seam texels on both sides are reached`, await value(`(()=>{const d=${E}.docs.get('mario_all');const c=window.__mkss.composite(d);const r=(x,y)=>c[(y*128+x)*4]===255&&c[(y*128+x)*4+1]===0;return [47,46].some(x=>r(x,46))&&[112,113].some(x=>r(x,46));})()`), "");
  check(`${T} mirrored occurrences highlighted while painting`, mirrorDuring.tris > 0 && mirrorDuring.hint, mirrorDuring);
  check(`${T} mirrored hint is an outline, not a painted layer`, mirrorDuring.outline, mirrorDuring);
  await base();
  await value(`(()=>{const e=document.getElementById('o-mirror');e.checked=false;e.dispatchEvent(new Event('input'));})()`);
  await drag(sCD.slice(0, 9).concat(sCD.slice(9)).map((p) => p), "left");
  await sleep(150);
  check(`${T} mirrored outline can be turned off`, (await value(`${E}.preview.mirrorTriangles`)) === 0 && (await value(`localStorage.getItem('mkss.showMirror')==='0'`)), await value(`${E}.preview.mirrorTriangles`));
  await value(`(()=>{const e=document.getElementById('o-mirror');e.checked=true;e.dispatchEvent(new Event('input'));})()`);

  await base();
  await value(`${E}.color.set([0,200,0,255])`);
  await drag(await line([-10, 82, 4], [10, 82, 4], 8));
  await sleep(150);
  const eyeK = await value(`[0,1,2,3].map(i=>window.__diff('mario_eye.'+i).k)`);
  check(`${T} eye stroke updates all frames`, eyeK.every((k) => k > 10) && eyeK.every((k) => k === eyeK[0]), eyeK);
  check(`${T} eye stroke leaves the body`, (await value(`window.__diff('mario_all').k`)) === 0, "");
  const undo0 = await value(`${E}.history.undoStack.length`);
  await value(`${E}.undo()`);
  await sleep(120);
  check(`${T} one undo reverts the stroke on every eye frame`, (await value(`[0,1,2,3].map(i=>window.__diff('mario_eye.'+i).k)`)).every((k) => k === 0) && (await value(`${E}.history.undoStack.length`)) === undo0 - 1, "");
  await value(`${E}.redo()`);
  await value(`(()=>{const s=document.getElementById('eye-frame');s.value='mario_eye.2';s.dispatchEvent(new Event('change'));})()`);
  await base();
  await value(`${E}.color.set([0,0,0,255])`);
  await drag(await line([-12, 76, 4], [12, 76, 4], 8));
  await sleep(150);
  const one = await value(`[0,1,2,3].map(i=>window.__diff('mario_eye.'+i).k)`);
  check(`${T} single eye frame mode paints only that frame`, one[2] > 0 && one[0] === 0 && one[1] === 0 && one[3] === 0, one);
  await value(`(()=>{const s=document.getElementById('eye-frame');s.value='all';s.dispatchEvent(new Event('change'));})()`);
  await click("#eye-blink");
  const eyeHashes = new Set();
  for (let k = 0; k < 5; k++) {
    eyeHashes.add(await value(`(()=>{const d=${E}.preview.textures.get('mario_eye.0').image.data;let h=0;for(let i=0;i<d.length;i+=7)h=(h*31+d[i])|0;return h;})()`));
    await sleep(820);
  }
  check(`${T} preview expressions really changes the eyes on the model`, eyeHashes.size >= 2 && (await value(`/\\d/.test(document.getElementById('eye-hint').textContent)`)), [...eyeHashes]);
  check(`${T} preview blink cycles frames`, await value(`${E}.blink && !!document.getElementById('eye-blink').checked`), "");
  await click("#eye-blink");
  await shot("22-paint3d-eyes-light-en");

  await base();
  await value(`${E}.setTool('fill');${E}.color.set([0,0,255,255]);${E}.opts.fillOpacity=1`);
  const pB = await scr([30, 60, 0]);
  await drag([pB]);
  await sleep(150);
  const dF = await value(`window.__diff('mario_all',[[0,0,64,64],[64,0,128,64]])`);
  check(`${T} fill on the model fills the part under the cursor`, dF.inR[1] > 1500 && dF.inR[0] < 200, dF);
  await value(`${E}.setTool('picker')`);
  const pA = await scr([-50, 20, 0]);
  await drag([pA]);
  const want = await value(`(()=>{const d=${E}.docs.get('mario_all');const s=${E}.preview.subMeshes[0];const h=${E}.preview.hitAt(${pA[0]},${pA[1]});const x=Math.floor(h.uv[0]*128),y=Math.floor(h.uv[1]*64);const c=window.__mkss.composite(d);return Array.from(c.slice((y*128+x)*4,(y*128+x)*4+4));})()`);
  check(`${T} eyedropper on the model`, (await value(`${E}.color.primary`)).join() === want.join(), { want, got: await value(`${E}.color.primary`), tool: await value(`${E}.tool`) });
  await value(`${E}.setTool('wand')`);
  await drag([pB]);
  await sleep(120);
  check(`${T} clicking the model selects the part`, await value(`!!${E}.focus && ${E}.focus.index===1 && ${E}.current.selection && ${E}.current.selection[100]===255 && ${E}.current.selection[10]===0`), await value(`${E}.focus`));
  check(`${T} orbit with right drag does not paint`, await (async () => {
    await base();
    const c = await scr([0, 30, 0]);
    await drag([c, [c[0] + 60, c[1] + 10], [c[0] + 120, c[1] + 20]], "right");
    return (await value(`window.__diff('mario_all').k`)) === 0;
  })(), "");
  await value(`${E}.setSelection(null)`);

  await toAdvanced();
  await value(`${E}.select('mario_all')`);
  await sleep(150);
  check(`${T} Advanced: unused texels dimmed`, (await value(`${E}.view.unusedCount`)) === 32 * 16, await value(`${E}.view.unusedCount`));
  check(`${T} Advanced: friendly texture names`, await value(`document.getElementById('tex-switch-name').textContent==='Body' && document.getElementById('tex-switch-meta').textContent.startsWith('mario_all')`), "");
  await click("#tex-switch");
  check(`${T} Advanced: switcher shows Body / low detail / eye frames`, await value(`(()=>{const t=[...document.querySelectorAll('#tex-pop li b')].map(b=>b.textContent);return t.includes('Body')&&t.includes('Body (low detail)')&&t.includes('Eyes (frame 2)');})()`), await value(`[...document.querySelectorAll('#tex-pop li b')].map(b=>b.textContent)`));
  await shot("23-advanced-switcher-light-en");
  await click("#tex-pop li[data-name='mario_all']");
  const pu = await texel(110, 6);
  await mouse("mouseMoved", pu[0], pu[1], "none");
  await sleep(80);
  check(`${T} Advanced: hover on unused texel says not shown in game`, await value(`document.getElementById('st-part').textContent==='Not shown in game' && ${E}.view.canvas.title==='Not shown in game'`), await value(`document.getElementById('st-part').textContent`));
  await value(`${E}.setTool('brush');${E}.opts.brush.size=3`);
  await drag([pu, [pu[0] + 4, pu[1]]]);
  check(`${T} Advanced: painting unused texels allowed with a status warning`, await value(`document.getElementById('st-warn').textContent.length>10`), "");
  await shot("24-advanced-unused-light-en");

  await value("window.noEyes=true");
  await leaveTo('characters', 'discard');
  await click("#char-grid button.char[data-code=fk]");
  await wait(`!document.getElementById('view-edit').classList.contains('hidden') && ${E}.bundle && ${E}.bundle.eyes.length===0 && ${E}.mode==='3d'`);
  await sleep(200);
  check(`${T} character without eye frames hides eye controls`, await value(`document.getElementById('eye-panel').classList.contains('hidden')`), "");
  await value("window.noEyes=false");
}

async function handAndOutfits() {
  await boot("light", "en", 1280, 720);
  await toEditor();
  const T = "v031";
  const cam = () => value(`${E}.preview.cam.position.toArray().map(v=>+v.toFixed(3))`);
  const tgt = () => value(`${E}.preview.target.toArray().map(v=>+v.toFixed(3))`);
  const hash = () => value(`(()=>{let h=0;for(const d of ${E}.docs.values()){const c=${E}.pixelsOf(d);for(let i=0;i<c.length;i++)h=(h*31+c[i])>>>0;}return h;})()`);
  const centre = () => value(`(()=>{const r=${E}.preview.renderer.domElement.getBoundingClientRect();return [r.left+r.width/2,r.top+r.height/2];})()`);
  const path = (c, dx = 120, dy = 30, n = 6) => Array.from({ length: n + 1 }, (_, i) => [c[0] - 40 + (dx * i) / n, c[1] - 20 + (dy * i) / n]);
  const reset = async () => { await value(`${E}.preview.resetView()`); await sleep(60); };
  const space = (type) => send("Input.dispatchKeyEvent", { type, key: " ", code: "Space", windowsVirtualKeyCode: 32, text: type === "keyDown" ? " " : undefined });
  check(`${T} Kart/Bike hidden for a character whose outfits are the same`, await value(`document.getElementById('vehicle-group').classList.contains('hidden') || document.getElementById('ed-outfit').classList.contains('hidden')`), "");
  await value(`document.activeElement?.blur?.();${E}.color.set([255,0,0,255]);${E}.opts.size3d.brush=24`);
  await key("h", "KeyH", 72);
  check(`${T} H selects the Hand tool (in the left tool bar)`, await value(`${E}.tool==='hand' && document.querySelector('#ed-tools [data-tool=hand]').getAttribute('aria-pressed')==='true' && !document.querySelector('#ed-tools [data-tool=hand]').classList.contains('hidden')`), await value(`${E}.tool`));
  check(`${T} Hand hint + Reset view in tool options`, await value(`document.getElementById('hand-hint').textContent.includes('rotate') && !!document.getElementById('o-reset')`), "");
  check(`${T} Hand shows a grab cursor`, (await value(`getComputedStyle(${E}.preview.renderer.domElement).cursor`)) === "grab", await value(`getComputedStyle(${E}.preview.renderer.domElement).cursor`));
  const c = await centre();
  await reset();
  let a = await cam(), at = await tgt(), h0 = await hash();
  await drag(path(c));
  check(`${T} Hand: left drag rotates the model and does not paint`, (await cam()).join() !== a.join() && (await tgt()).join() === at.join() && (await hash()) === h0, { a, b: await cam() });
  await reset();
  a = await cam(); at = await tgt();
  await drag(path(c), "left", 8);
  check(`${T} Hand: Shift+drag moves (pans) the model`, (await tgt()).join() !== at.join() && (await hash()) === h0, { at, bt: await tgt() });
  await reset();
  at = await tgt();
  await drag(path(c), "right");
  check(`${T} Hand: right drag moves (pans) the model`, (await tgt()).join() !== at.join() && (await hash()) === h0, "");
  const moved = await cam();
  await click("#o-reset");
  check(`${T} Reset view restores the camera`, (await cam()).join() !== moved.join() && (await tgt()).join() === at.join(), "");
  check(`${T} Reset view also labelled under the 3D view`, await value(`document.getElementById('btn-p3d-reset').textContent.trim()==='Reset view'`), "");
  a = await cam();
  await send("Input.dispatchMouseEvent", { type: "mouseWheel", x: c[0], y: c[1], deltaX: 0, deltaY: -300 });
  await sleep(120);
  check(`${T} wheel zooms`, (await cam()).join() !== a.join(), "");
  await key("b", "KeyB", 66);
  for (const [btn, mod, name] of [["right", 0, "right"], ["middle", 0, "middle"], ["left", 1, "Alt+left"]]) {
    await reset();
    a = await cam();
    await drag(path(c), btn, mod);
    check(`${T} Brush active: ${name} drag rotates, no paint`, (await cam()).join() !== a.join() && (await hash()) === h0, { name, tool: await value(`${E}.tool`) });
  }
  await reset();
  a = await cam();
  await drag(path(c), "right", 8);
  check(`${T} Brush active: Shift+right drag pans`, (await tgt()).join() !== at.join(), "");
  await reset();
  a = await cam();
  await space("keyDown");
  await sleep(60);
  check(`${T} holding Space shows the hand cursor`, (await value(`getComputedStyle(${E}.preview.renderer.domElement).cursor`)) === "grab" && (await value(`${E}.tool`)) === "brush", "");
  await drag(path(c));
  check(`${T} Space+left drag rotates instead of painting`, (await cam()).join() !== a.join() && (await hash()) === h0, "");
  await space("keyUp");
  await sleep(60);
  await reset();
  await drag(path(c));
  check(`${T} after releasing Space, left drag paints again`, (await hash()) !== h0, "");
  const h1 = await hash();
  await mouse("mouseMoved", c[0] - 60, c[1], "none");
  await mouse("mousePressed", c[0] - 60, c[1], "left");
  await mouse("mouseMoved", c[0] - 40, c[1], "left");
  const mid = await hash();
  await value(`${E}.preview.onNavStart()`);
  await mouse("mouseReleased", c[0] - 40, c[1], "left");
  await sleep(80);
  check(`${T} stroke cancelled when the press becomes a view drag`, mid !== h1 && (await hash()) === h1, { mid, h1 });
  await shot("30-hand-tool-light-en");

  await leaveTo('characters', 'discard');
  await click("#char-grid button.char[data-code=pc]");
  await wait(`!document.getElementById('view-edit').classList.contains('hidden') && ${E}.character.code==='pc' && ${E}.bundle && ${E}.bundle.models.length===4`);
  await sleep(300);
  const st = () => value(`(()=>({veh:${E}.vehicle,det:${E}.detail,model:${E}.preview.current&&${E}.preview.current.name+'@'+${E}.preview.current.file,tex:${E}.preview.subMeshes.map(s=>s.texName),cur:${E}.current&&${E}.current.name,parts:[...document.querySelectorAll('#parts-list li[data-i] span')].map(s=>s.textContent),label:document.getElementById('p3d-label').textContent,low:!document.getElementById('low-hint').classList.contains('hidden'),eyes:!document.getElementById('eye-panel').classList.contains('hidden'),dots:['veh-kart','veh-bike','det-full','det-low'].map(id=>!document.querySelector('#'+id+' .seg-dot').classList.contains('hidden'))}))()`);
  let s0 = await st();
  check(`${T} Kart/Bike + Full/Low shown for a character with two outfits`, await value(`!document.getElementById('ed-outfit').classList.contains('hidden') && !document.getElementById('vehicle-group').classList.contains('hidden') && document.getElementById('veh-kart').getAttribute('aria-checked')==='true' && document.getElementById('det-full').getAttribute('aria-checked')==='true'`), s0);
  check(`${T} starts on Kart · Full detail`, s0.tex.includes("mario_all") && s0.cur === "mario_all" && s0.parts.length === 2 && s0.parts[0].startsWith("Head") && s0.label === "Kart · Full detail" && !s0.low && s0.eyes, s0);
  await value(`document.getElementById('parts-list').querySelector('li[data-i="1"]').click()`);
  check(`${T} a part is selected on the kart outfit`, await value(`!!${E}.focus && !!${E}.current.selection`), "");
  await click("#veh-bike");
  let s1 = await st();
  check(`${T} Bike switches model, textures and the Body parts list`, s1.veh === "bike" && s1.tex.includes("bike_suit_all") && !s1.tex.includes("mario_all") && s1.cur === "bike_suit_all" && s1.parts.length === 2 && s1.parts.every((p) => /^(Dress|Legs)/.test(p)) && s1.label === "Bike · Full detail", s1);
  check(`${T} switching clears the old part selection`, await value(`!${E}.focus && !${E}.docs.get('mario_all').selection && !document.querySelector('#parts-list li.on')`), "");
  await reset();
  await value(`window.__base=(n)=>{window.__b=window.__b||{};window.__b[n]=Array.from(${E}.pixelsOf(${E}.docs.get(n)));};window.__k=(n)=>{const c=${E}.pixelsOf(${E}.docs.get(n));const b=window.__b[n];let k=0;for(let i=0;i<c.length;i+=4)if(c[i]!==b[i]||c[i+1]!==b[i+1]||c[i+2]!==b[i+2]||c[i+3]!==b[i+3])k++;return k;};['mario_all','bike_suit_all','mario_tx_64','bike_suit_tx_6432'].forEach(n=>window.__base(n))`);
  const scr = (p) => value(`${E}.preview.screenOf(${JSON.stringify(p)})`);
  const stroke = async (p0, p1) => { const A = await scr(p0), B = await scr(p1); await drag(Array.from({ length: 9 }, (_, i) => [A[0] + ((B[0] - A[0]) * i) / 8, A[1] + ((B[1] - A[1]) * i) / 8])); await sleep(150); };
  await value(`${E}.color.set([0,0,255,255]);${E}.setTool('brush')`);
  await stroke([-45, 40, 0], [-20, 40, 0]);
  let k = await value(`['mario_all','bike_suit_all','mario_tx_64','bike_suit_tx_6432'].map(window.__k)`);
  check(`${T} painting the Bike outfit edits only the bike textures`, k[1] > 20 && k[0] === 0 && k[3] > 0 && k[2] === 0, k);
  await sleep(350);
  s1 = await st();
  check(`${T} edited dot on Bike only`, !s1.dots[0] && s1.dots[1], s1.dots);
  await shot("31-outfit-bike-full-light-en");
  await click("#veh-kart");
  let s2 = await st();
  check(`${T} back to Kart: kart model, mario_all parts, bike edit kept`, s2.tex.includes("mario_all") && s2.cur === "mario_all" && s2.parts[0].startsWith("Head") && (await value(`window.__k('bike_suit_all')`)) === k[1], s2);
  await reset();
  await stroke([-45, 40, 0], [-20, 40, 0]);
  await sleep(350);
  k = await value(`['mario_all','bike_suit_all','mario_tx_64','bike_suit_tx_6432'].map(window.__k)`);
  s2 = await st();
  check(`${T} Kart edit lands on the kart textures; both outfits show an edited dot`, k[0] > 20 && k[2] > 0 && s2.dots[0] && s2.dots[1], { k, dots: s2.dots });
  await click("#det-low");
  let s3 = await st();
  check(`${T} Low detail shows the low-detail model with *_tx_64`, s3.det === "low" && s3.model === "model_lod@Race/Kart/ma_kart-pc.szs" && s3.tex.join() === "mario_tx_64" && s3.cur === "mario_tx_64" && s3.parts.length === 1 && s3.parts[0].startsWith("Head") && s3.label === "Kart · Low detail", s3);
  check(`${T} Low detail hint shown, eye controls hidden`, s3.low && !s3.eyes && (await value(`document.getElementById('low-hint').textContent.includes('far away')`)), s3);
  await reset();
  await value(`${E}.color.set([0,255,0,255])`);
  await value(`window.__base('mario_tx_64')`);
  await stroke([-25, 40, 0], [-5, 40, 0]);
  const lowK = await value(`window.__k('mario_tx_64')`);
  const exact = await value(`(()=>{const d=${E}.docs.get('mario_tx_64');const c=${E}.pixelsOf(d);let n=0;for(let i=0;i<c.length;i+=4)if(c[i]===0&&c[i+1]===255&&c[i+2]===0&&c[i+3]===255)n++;return n;})()`);
  check(`${T} painting in Low detail edits the low texture directly (overrides the derived texels)`, lowK > 10 && exact > 10 && (await value(`${E}.edited('mario_tx_64') && !${E}.summary(${E}.docs.get('mario_tx_64')).derivedFrom`)), { lowK, exact });
  await sleep(350);
  check(`${T} Low detail edited dot`, (await st()).dots[3], "");
  await shot("32-outfit-kart-low-light-en");
  await click("#veh-bike");
  const s4 = await st();
  check(`${T} Bike + Low detail: bike LOD model, its texture and parts`, s4.model === "model_lod@Race/Kart/ma_bike-pc.szs" && s4.tex.join() === "bike_suit_tx_6432" && s4.cur === "bike_suit_tx_6432" && s4.parts.length === 1 && s4.parts[0].startsWith("Torso") && s4.label === "Bike · Low detail", s4);
  await click("#det-full");
  const s5 = await st();
  check(`${T} Bike + Full detail again: parts list follows`, s5.cur === "bike_suit_all" && s5.parts.every((p) => /^(Dress|Legs)/.test(p)) && s5.eyes && !s5.low, s5);
  await toAdvanced();
  check(`${T} Advanced mode keeps the outfit (bike body texture)`, await value(`${E}.current.name==='bike_suit_all' && document.getElementById('tex-switch-name').textContent==='Body (bike outfit)' && document.getElementById('veh-bike').getAttribute('aria-checked')==='true'`), await value(`document.getElementById('tex-switch-name').textContent`));
  await click("#veh-kart");
  check(`${T} Advanced: Kart selects the kart body and its parts`, await value(`${E}.current.name==='mario_all' && document.getElementById('tex-switch-name').textContent==='Body (kart outfit)' && document.querySelector('#parts-list li[data-i] span').textContent.startsWith('Head')`), "");
  await click("#mode-3d");
  await sleep(150);
  await value(`calls.length=0`);
  await value(`${E}.commitAll()`);
  const committed = await value(`calls.filter(c=>c.cmd==='commit_texture'&&c.args.png).map(c=>c.args.name).sort()`);
  check(`${T} export commits the kart and bike outfit edits`, ["bike_suit_all", "bike_suit_tx_6432", "mario_all", "mario_tx_64"].every((n) => committed.includes(n)), committed);
  for (const [w, h] of [[1280, 720], [1440, 900]]) {
    await size(w, h);
    await click("#det-low");
    const probe = await value(layoutProbe);
    check(`${T} ${w}x${h} outfit bar + low hint: no scroll/clipping`, probe.length === 0, probe.slice(0, 5));
    await click("#det-full");
  }
  await size(1280, 720);
}

async function sharedOutfits() {
  await boot("light", "en", 1280, 720);
  await newProject("Shared");
  await openChar("pc");
  await wait(`${E}.bundle && ${E}.bundle.models.length===4`);
  const T = "v041";
  const NAMES4 = "['mario_all','bike_suit_all','mario_tx_64','bike_suit_tx_6432']";
  await value(`window.__base=(n)=>{window.__b=window.__b||{};window.__b[n]=Array.from(${E}.pixelsOf(${E}.docs.get(n)));};window.__k=(n)=>{const c=${E}.pixelsOf(${E}.docs.get(n));const b=window.__b[n];let k=0;for(let i=0;i<c.length;i+=4)if(c[i]!==b[i]||c[i+1]!==b[i+1]||c[i+2]!==b[i+2]||c[i+3]!==b[i+3])k++;return k;};window.__where=(n)=>{const c=${E}.pixelsOf(${E}.docs.get(n));const b=window.__b[n];const out=[];for(let i=0;i<c.length;i+=4)if(c[i]!==b[i]||c[i+1]!==b[i+1]||c[i+2]!==b[i+2]||c[i+3]!==b[i+3])out.push([(i/4)%128,Math.floor(i/4/128)]);return out;};${NAMES4}.forEach(n=>window.__base(n))`);
  const ks = () => value(`${NAMES4}.map(window.__k)`);
  const dots = () => value(`['veh-kart','veh-bike'].map(id=>!document.querySelector('#'+id+' .seg-dot').classList.contains('hidden'))`);
  const scr = (p) => value(`${E}.preview.screenOf(${JSON.stringify(p)})`);
  const stroke = async (p0, p1) => { const A = await scr(p0), B = await scr(p1); await drag(Array.from({ length: 9 }, (_, i) => [A[0] + ((B[0] - A[0]) * i) / 8, A[1] + ((B[1] - A[1]) * i) / 8])); await sleep(150); };
  check(`${T} note under Vehicle: face and hair are shared`, await value(`(()=>{const n=document.getElementById('outfit-shared');const v=document.getElementById('vehicle-group').getBoundingClientRect();const r=n.getBoundingClientRect();return !n.classList.contains('hidden') && n.textContent==='Face and hair are shared: edits appear on both Kart and Bike.' && r.top>=v.bottom-1 && Math.abs(r.left-v.left)<12 && r.height>0 && r.height<20;})()`), await value(`document.getElementById('outfit-shared').getBoundingClientRect().toJSON()`));
  await value(`${E}.preview.resetView();${E}.color.set([255,0,0,255]);${E}.setTool('brush');${E}.opts.size3d.brush=14`);
  await stroke([-50, 85, 0], [-20, 85, 0]);
  await sleep(350);
  let k = await ks();
  const whereBike = await value(`window.__where('bike_suit_all')`);
  check(`${T} hair painted on Kart: Bike atlas follows (linked texels only), bike low detail too`, k[0] > 20 && k[1] > 20 && k[3] > 0 && whereBike.every(([x, y]) => x < 64 && y < 24), { k, outside: whereBike.filter(([x, y]) => x >= 64 || y >= 24).slice(0, 5) });
  check(`${T} Bike texels = bike original + kart change (delta), bike not hand-edited, derivedFrom kart`, await value(`(()=>{const b=${E}.docs.get('bike_suit_all');const px=${E}.pixelsOf(b);const k=${E}.docs.get('mario_all');const kp=${E}.pixelsOf(k);let ok=0,n=0;for(const [x,y] of window.__where('bike_suit_all')){const i=(y*128+x)*4;n++;let good=true;for(let c=0;c<4;c++){const want=Math.max(0,Math.min(255,b.original[i+c]+kp[i+c]-k.original[i+c]));if(Math.abs(px[i+c]-want)>1)good=false;}if(good)ok++;}return n>20 && ok===n && ${E}.summary(b).derivedFrom==='mario_all' && ${E}.edited('bike_suit_all');})()`), await value(`${E}.summary(${E}.docs.get('bike_suit_all'))`));
  check(`${T} Bike edited dot reflects the shared (derived) edit`, (await dots()).join() === "true,true", await dots());
  await click("#veh-bike");
  await sleep(200);
  check(`${T} Bike 3D texture = derived pixels (propagated hair visible)`, await value(`(()=>{const t=${E}.preview.textures.get('bike_suit_all');const px=${E}.pixelsOf(${E}.docs.get('bike_suit_all'));if(!t)return false;const d=t.image.data;let diff=0;for(let i=0;i<px.length;i++)if(d[i]!==px[i])diff++;return diff===0 && ${E}.preview.subMeshes.some(s=>s.texName==='bike_suit_all');})()`), "");
  await shot("33-shared-hair-bike-light-en");
  await value(`${E}.preview.resetView();${E}.color.set([0,255,0,255])`);
  await stroke([-50, 85, 0], [-20, 85, 0]);
  await sleep(350);
  check(`${T} bike paint overrides the derived texels, kart own paint kept`, await value(`(()=>{const b=${E}.pixelsOf(${E}.docs.get('bike_suit_all'));const kp=${E}.pixelsOf(${E}.docs.get('mario_all'));let g=0,kr=0;for(let i=0;i<b.length;i+=4){if(b[i]===0&&b[i+1]===255&&b[i+2]===0)g++;if(kp[i]===255&&kp[i+1]===0&&kp[i+2]===0)kr++;}return g>20 && kr>20 && !${E}.summary(${E}.docs.get('bike_suit_all')).derivedFrom;})()`), "");
  await value(`window.__base('mario_all')`);
  await value(`${E}.color.set([0,0,255,255])`);
  await stroke([-50, 96, 0], [-20, 96, 0]);
  await sleep(350);
  const whereKart = await value(`window.__where('mario_all')`);
  check(`${T} hair painted on Bike: Kart follows (linked texels only)`, whereKart.length > 10 && whereKart.every(([x, y]) => x < 64 && y < 24), whereKart.length);
  await click("#veh-kart");
  await sleep(200);
  check(`${T} Kart 3D texture shows the bike's hair edit`, await value(`(()=>{const t=${E}.preview.textures.get('mario_all');const px=${E}.pixelsOf(${E}.docs.get('mario_all'));const d=t.image.data;for(let i=0;i<px.length;i++)if(d[i]!==px[i])return false;return true;})()`), "");
  await click("#btn-undo");
  await sleep(120);
  check(`${T} undo (bike stroke 2) removes it from the Kart too`, (await value(`window.__k('mario_all')`)) === 0, await value(`window.__k('mario_all')`));
  await click("#btn-undo");
  await click("#btn-undo");
  await sleep(350);
  await value(`window.__base('mario_all')`);
  k = await value(`['bike_suit_all','bike_suit_tx_6432'].map(window.__k)`);
  check(`${T} undo all: Bike atlas + bike low detail back to the original, no edited dots`, await value(`!${E}.edited('bike_suit_all') && !${E}.edited('bike_suit_tx_6432') && !${E}.edited('mario_all')`) && (await dots()).join() === "false,false", { k, dots: await dots() });
  const parts = () => value(`[...document.querySelectorAll('#parts-list li[data-i]')].map(li=>({i:li.dataset.i,on:li.classList.contains('on'),sw:li.querySelector('.sw').style.background,label:li.querySelector('span').textContent}))`);
  const p0 = await parts();
  await click("#parts-list li[data-i='0']");
  await value(`${E}.color.set([20,40,230,255]);${E}.setTool('fill')`);
  const pt = await scr([-30, 50, 0]);
  await mouse("mouseMoved", pt[0], pt[1], "none");
  await mouse("mousePressed", pt[0], pt[1], "left");
  await mouse("mouseReleased", pt[0], pt[1], "left");
  await wait(`document.querySelector("#parts-list li[data-i='0'] span").textContent!==${JSON.stringify(p0[0].label)}`).catch(() => {});
  const p1 = await parts();
  check(`${T} parts: swatch and colour word follow the recolour (Head · Red -> Head · Blue), order unchanged`, p0[0].label === "Head · Red" && p1[0].sw === "rgb(20, 40, 230)" && p1[0].label === "Head · Blue" && p1[1].label === p0[1].label && p1[1].sw === p0[1].sw && p1.map((x) => x.i).join() === p0.map((x) => x.i).join() && p0[0].label !== p1[0].label, { p0, p1 });
  check(`${T} parts: selection stays on the same part`, p1[0].on && (await value(`${E}.focus && ${E}.focus.index===0 && !!${E}.current.selection`)), p1);
  check(`${T} parts: the recolour also reached the Bike hair (shared)`, (await value(`window.__k('bike_suit_all')`)) > 100, "");
  await click("#btn-undo");
  await wait(`document.querySelector("#parts-list li[data-i='0'] span").textContent===${JSON.stringify(p0[0].label)}`).catch(() => {});
  const p2 = await parts();
  check(`${T} parts: undo restores swatch and name, selection kept`, p2[0].sw === p0[0].sw && p2[0].label === p0[0].label && p2[0].on, { p0: p0[0], p2: p2[0] });
  await click("#veh-bike");
  await sleep(300);
  const pb = await parts();
  check(`${T} parts: outfit switch shows the bike parts with their current colours`, pb.length === 2 && /^(Dress|Legs)/.test(pb[0].label), pb);
  for (const [w, h] of [[1280, 720], [1440, 900]]) {
    await size(w, h);
    const probe = await value(layoutProbe);
    check(`${T} ${w}x${h} outfit bar with the shared note: no scroll/clipping`, probe.length === 0, probe.slice(0, 5));
  }
  await size(1280, 720);
}

async function projectFlow() {
  await boot("light", "en", 1280, 720);
  const T = "v04";
  const P = `${S}.project`;
  const stroke3d = async (p0, p1, rgb = [255, 0, 0]) => {
    await value(`${E}.preview.resetView();${E}.setTool('brush');${E}.color.set([${rgb},255]);${E}.opts.size3d.brush=24`);
    await sleep(80);
    const A = await value(`${E}.preview.screenOf(${JSON.stringify(p0)})`), B = await value(`${E}.preview.screenOf(${JSON.stringify(p1)})`);
    await drag(Array.from({ length: 9 }, (_, i) => [A[0] + ((B[0] - A[0]) * i) / 8, A[1] + ((B[1] - A[1]) * i) / 8]));
    await sleep(350);
  };
  check(`${T} app starts on Projects`, await value(`${S}.tab==='projects' && document.querySelector('.app-tabs [data-tab=projects]').getAttribute('aria-current')==='true'`), "");
  check(`${T} sidebar contains only Projects`, await value("[...document.querySelectorAll('.app-tabs button')].map(b=>b.dataset.tab).join()==='projects' && !document.querySelector('.steps, .app-tabs .n')"), "");
  check(`${T} empty state explains a project with a big New project button`, await value("!document.getElementById('proj-empty').classList.contains('hidden') && document.getElementById('proj-empty').textContent.includes('mod') && document.getElementById('btn-new-project-empty').classList.contains('big') && document.getElementById('btn-new-project').classList.contains('hidden')"), "");
  check(`${T} project-only screens are not global sidebar destinations`, await value("!document.querySelector('.app-tabs [data-tab=characters],.app-tabs [data-tab=studio]') && document.querySelector('.sidebar').getBoundingClientRect().width>=190"), 'home keeps expanded branding');
  await click(".app-tabs [data-tab=projects]");
  check(`${T} Projects home remains stable on its own nav action`, await value(`${S}.tab==='projects' && !${S}.project`), "");
  await shot("40-projects-empty-light-en");
  await toNew();
  check(`${T} New project proposes a default name`, await value("document.getElementById('new-name').value==='My skin pack 1' && document.querySelector('.app-tabs [data-tab=projects]').getAttribute('aria-current')==='true'"), await value("document.getElementById('new-name').value"));
  await shot("41-new-project-light-en");
  await newProject("Team colours");
  check(`${T} project created with name and game`, await value("(()=>{const c=calls.find(c=>c.cmd==='project_create');return c && c.args.name==='Team colours' && c.args.gamePath==='D:/Games/My game copy/DATA/files' && c.args.game==='My game copy';})()"), "");
  check(`${T} Characters tab: 24 characters, none edited, hint and project name`, await value("document.querySelectorAll('#char-grid button.char').length===24 && !document.querySelector('.char-cell.edited') && document.querySelector('.chars-hint').textContent.includes('export the mod here') && document.getElementById('proj-name').value==='Team colours' && document.getElementById('proj-saved').textContent.includes('Saved')"), "");
  check(`${T} Export disabled with an explanation when nothing is edited`, await value("document.getElementById('btn-export-mod').disabled && document.getElementById('btn-export-mod').classList.contains('primary') && document.getElementById('export-why').textContent==='Edit at least one character to export.'"), "");
  check(`${T} project opens with compact sidebar and no Studio destination`, await value("document.body.classList.contains('project-open') && !document.body.classList.contains('editing') && document.querySelector('.sidebar').getBoundingClientRect().width===60 && !document.querySelector('.app-tabs [data-tab=studio]')"), 'select a character from the project grid');
  await openChar('mr');
  check(`${T} Studio header: Project › Character breadcrumb, Saved, Done`, await value("document.getElementById('crumb-project').textContent==='Team colours' && document.getElementById('crumb-char').textContent==='Mario' && document.getElementById('save-state').textContent==='Saved' && document.getElementById('btn-done').classList.contains('primary') && document.querySelectorAll('#view-edit button.primary:not(.mini)').length===1"), "");
  check(`${T} Save button disabled while everything is saved`, await value("document.getElementById('btn-proj-save').disabled && document.getElementById('btn-proj-save').textContent.includes('Save changes')"), "");
  check(`${T} Studio has no export button`, await value("!document.getElementById('btn-build') && !document.getElementById('btn-to-export') && !document.getElementById('btn-export-mod').closest('#view-edit')"), "");
  await stroke3d([-45, 40, 0], [-20, 40, 0]);
  check(`${T} painting marks unsaved changes and enables Save`, await value("window.__mkss.dirty() && document.getElementById('save-state').textContent==='Unsaved changes' && !document.getElementById('btn-proj-save').disabled"), "");
  await shot("42-studio-unsaved-light-en");
  await click("#btn-done");
  check(`${T} Done with unsaved changes asks Save changes / Discard / Cancel`, await value("document.getElementById('ask-dialog').open && [...document.querySelectorAll('#ask-actions button')].map(b=>b.dataset.id).join()==='cancel,discard,save' && document.querySelector('#ask-actions button.primary').dataset.id==='save' && document.querySelector('#ask-actions button.primary').textContent==='Save changes' && document.getElementById('ask-title').textContent.includes('Mario')"), "");
  await shot("43-unsaved-dialog-light-en");
  await answer('cancel');
  check(`${T} Cancel stays in the Studio with the changes`, await value(`${S}.tab==='studio' && window.__mkss.dirty() && !calls.some(c=>c.cmd==='project_char_write')`), "");
  await click("#crumb-project");
  check(`${T} switching tab with unsaved changes asks too`, await askOpen(), "");
  await answer('discard');
  await tab('characters');
  check(`${T} Discard: nothing saved, Mario not edited, Studio closed`, await value(`!calls.some(c=>c.cmd==='project_char_write') && !document.querySelector('.char-cell.edited') && !${S}.character && document.getElementById('view-edit').classList.contains('hidden')`), "");
  await openChar('mr');
  check(`${T} discarded edits are gone when reopening`, await value(`${E}.editedDocs().length===0 && !window.__mkss.dirty()`), "");
  await stroke3d([-45, 40, 0], [-20, 40, 0]);
  await click("#btn-done");
  await answer('save');
  await tab('characters');
  check(`${T} Save: written into the project, Mario marked Edited with what changed`, await value(`(()=>{const c=calls.filter(c=>c.cmd==='project_char_write').at(-1);const cell=document.querySelector('[data-code=mr]').closest('.char-cell');return c && c.args.code==='mr' && c.args.edits.join()==='body' && cell.classList.contains('edited') && cell.querySelector('.b.ed').textContent==='Edited' && cell.querySelector('.char-status small').textContent==='Body' && !!cell.querySelector('.char-revert');})()`), await value("calls.filter(c=>c.cmd==='project_char_write').map(c=>c.args.edits)"));
  check(`${T} un-edited characters stay neutral without revert`, await value("(()=>{const cell=document.querySelector('[data-code=pc]').closest('.char-cell');return !cell.classList.contains('edited') && !cell.querySelector('.char-revert') && cell.querySelector('.char-status').textContent==='Original';})()"), "");
  check(`${T} Export enabled once a character is edited`, await value("!document.getElementById('btn-export-mod').disabled && document.getElementById('export-why').textContent.includes('1')"), "");
  check(`${T} saved character remains available through project grid`, await value("window.__mkss.state.character?.code==='mr' && !!document.querySelector('#char-grid [data-code=mr]')"), 'no global Studio nav is needed');
  await openChar('pc');
  await stroke3d([-45, 40, 0], [-20, 40, 0], [0, 0, 255]);
  await click("#veh-bike");
  await stroke3d([-45, 40, 0], [-20, 40, 0], [0, 255, 0]);
  await stroke3d([-10, 82, 4], [10, 82, 4], [255, 255, 0]);
  await key("s", "KeyS", 83, 2);
  await wait("calls.filter(c=>c.cmd==='project_char_write').length===2");
  await sleep(150);
  check(`${T} Ctrl+S saves into the project`, await value("!window.__mkss.dirty() && document.getElementById('save-state').textContent==='Saved'"), "");
  await click("#crumb-project");
  check(`${T} leaving a saved character does not ask`, await value(`!document.getElementById('ask-dialog').open && ${S}.tab==='characters'`), "");
  check(`${T} Peach badge lists body, eyes and bike outfit (+ low detail derived automatically is not hand painting)`, await value("document.querySelector('[data-code=pc]').closest('.char-cell').querySelector('.char-status small').textContent==='Body · Eyes · Bike outfit'"), await value("document.querySelector('[data-code=pc]').closest('.char-cell').textContent"));
  await shot("44-characters-edited-light-en");
  const before = await value("calls.length");
  await click("#btn-export-mod");
  await wait("!document.getElementById('export-done').classList.contains('hidden')");
  const ex = await value(`(()=>{const c=calls.slice(${before});return {cmds:c.map(x=>x.cmd).filter(x=>['pack_begin','load_character','pack_stage','build_pack'].includes(x)),loads:c.filter(x=>x.cmd==='load_character').map(x=>x.args.code),build:c.find(x=>x.cmd==='build_pack')?.args,pack:window.pack,low:c.some(x=>x.cmd==='commit_texture'&&x.args.name==='mario_tx_64'&&x.args.png&&x.args.summary.derivedFrom==='mario_all')};})()`);
  check(`${T} export flattens every edited character, then builds ONE pack`, ex.cmds.join() === "pack_begin,load_character,pack_stage,load_character,pack_stage,build_pack" && ex.loads.join() === "mr,pc", ex);
  check(`${T} build_pack called with both characters and the project name`, ex.build && ex.build.name === "Team colours" && ex.build.out.endsWith("Team colours.zip") && ex.pack.map((x) => x.code).join() === "mr,pc" && ex.pack[1].textures.includes("bike_suit_all") && ex.pack[1].textures.includes("mario_eye.0"), ex);
  check(`${T} low-detail textures derived per character in the export`, ex.low, ex);
  check(`${T} export result shown on the Characters tab`, await value("document.getElementById('export-done-text').textContent.startsWith('Mod exported · characters: 2 · files:') && document.getElementById('export-done-path').textContent.endsWith('Team colours.zip')"), await value("document.getElementById('export-done-text').textContent"));
  check(`${T} after export no character is open in the Studio`, await value(`!${S}.character && document.getElementById('view-edit').classList.contains('hidden')`), "");
  await shot("45-characters-exported-light-en");
  await click(".app-tabs [data-tab=projects]");
  await tab('projects');
  check(`${T} Projects card: name, 2 portraits, count, modified, game`, await value("(()=>{const c=document.querySelector('#proj-list .proj-card');return document.querySelectorAll('#proj-list .proj-card').length===1 && c.querySelector('.proj-name').textContent==='Team colours' && c.querySelectorAll('.proj-face img').length===2 && c.querySelector('.proj-meta').textContent.startsWith('Edited characters: 2 · Modified') && c.querySelector('.proj-game').textContent==='Game: My game copy' && c.classList.contains('current');})()"), await value("document.querySelector('#proj-list').textContent"));
  check(`${T} filled home: New project in the header is the only primary`, await value("!document.getElementById('btn-new-project').classList.contains('hidden') && document.getElementById('proj-empty').classList.contains('hidden') && [...document.querySelectorAll('#view-projects button.primary')].filter(b=>b.getClientRects().length).length===1"), "");
  await shot("46-projects-light-en");
  await click("#proj-list .proj-card [data-act=menu]");
  check(`${T} card menu lists the project actions`, await value("[...document.querySelectorAll('#proj-menu button')].map(b=>b.dataset.act).join()==='rename,duplicate,export,copy,delete' && !document.getElementById('proj-menu').classList.contains('hidden')"), "");
  await shot("47-project-menu-light-en");
  await click("#proj-menu [data-act=rename]");
  await wait("document.getElementById('ask-dialog').open");
  await value("document.getElementById('ask-input').value='Blue team'");
  await answer('ok');
  check(`${T} rename`, await value("document.querySelector('#proj-list .proj-name').textContent==='Blue team' && calls.some(c=>c.cmd==='project_rename'&&c.args.name==='Blue team')"), "");
  await click("#proj-list .proj-card [data-act=menu]");
  await click("#proj-menu [data-act=duplicate]");
  await wait("document.getElementById('ask-dialog').open");
  check(`${T} duplicate proposes a copy name`, await value("document.getElementById('ask-input').value==='Blue team (copy)'"), "");
  await answer('ok');
  check(`${T} duplicate adds a second card`, await value("document.querySelectorAll('#proj-list .proj-card').length===2 && [...document.querySelectorAll('#proj-list .proj-name')].map(e=>e.textContent).includes('Blue team (copy)')"), "");
  const copyCard = "[...document.querySelectorAll('#proj-list .proj-card')].find(c=>c.querySelector('.proj-name').textContent==='Blue team (copy)')";
  await value(`${copyCard}.querySelector('[data-act=menu]').click()`);
  await sleep(100);
  await click("#proj-menu [data-act=copy]");
  await wait("calls.some(c=>c.cmd==='project_export_file')");
  check(`${T} save a copy as .mkss`, await value("calls.find(c=>c.cmd==='project_export_file').args.path==='D:/Skins/copy.mkss'"), "");
  await value(`${copyCard}.querySelector('[data-act=menu]').click()`);
  await sleep(100);
  await click("#proj-menu [data-act=delete]");
  check(`${T} delete asks for confirmation`, await value("document.getElementById('ask-dialog').open && !!document.querySelector('#ask-actions button.danger')"), "");
  await answer('cancel');
  check(`${T} cancelled delete keeps the project`, await value("document.querySelectorAll('#proj-list .proj-card').length===2"), "");
  await value(`${copyCard}.querySelector('[data-act=menu]').click()`);
  await sleep(100);
  await click("#proj-menu [data-act=delete]");
  await answer('delete');
  check(`${T} delete removes the card`, await value("document.querySelectorAll('#proj-list .proj-card').length===1 && calls.some(c=>c.cmd==='project_delete')"), "");
  await click("#proj-list .proj-card [data-act=open]");
  await tab('characters');
  await click("[data-code=pc] ~ .char-revert");
  await wait("document.getElementById('ask-dialog').open");
  await answer('revert');
  check(`${T} revert removes Peach's edits from the project`, await value("!document.querySelector('[data-code=pc]').closest('.char-cell').classList.contains('edited') && calls.some(c=>c.cmd==='project_char_write'&&c.args.code==='pc'&&c.args.project===null) && document.getElementById('export-why').textContent.includes('1')"), "");
  await openChar('mr');
  await stroke3d([20, 40, 0], [45, 40, 0], [0, 128, 0]);
  await click("#window-close");
  check(`${T} title-bar close with unsaved changes asks first`, await value("document.getElementById('ask-dialog').open && !window.closedWindow"), "");
  await answer('cancel');
  check(`${T} Cancel keeps the window open and the changes`, await value("!window.closedWindow && window.__mkss.dirty()"), "");
  const writes = await value("calls.filter(c=>c.cmd==='project_char_write').length");
  await click("#window-close");
  await answer('save');
  await wait("window.closedWindow>0");
  check(`${T} Save then closes the window`, await value(`calls.filter(c=>c.cmd==='project_char_write').length===${writes + 1} && window.closedWindow===1`), "");
  await value(`(()=>{const m=JSON.parse(pstore[${P}.id].data.mr.manifest);store['D:/Skins/test.mkss']=pstore[${P}.id].data.mr;})()`);
  await value("window.__mkss.state.closeApproved=false");
  await leaveTo('projects');
  await click("#btn-import-project");
  await tab('characters');
  check(`${T} imported .mkss becomes a project with its character`, await value(`${P}.name==='test' && ${P}.characters.length===1 && document.querySelector('[data-code=mr]').closest('.char-cell').classList.contains('edited')`), await value(`${P}`));
  check(`${T} imported project is linked to the open game`, await value(`calls.some(c=>c.cmd==='project_set_game'&&c.args.id===${P}.id&&c.args.gamePath==='D:/Games/My game copy/DATA/files')`), "");
  await shot("48-imported-light-en");
  await click(".app-tabs [data-tab=projects]");
  await value("localStorage.setItem('karps.theme','dark');document.documentElement.dataset.theme='dark'");
  await sleep(200);
  await shot("50-projects-dark-en");
  await click("#proj-list .proj-card [data-act=open]");
  await tab('characters');
  await shot("51-characters-dark-en");
  await openChar('mr');
  await stroke3d([-45, 40, 0], [-20, 40, 0], [255, 0, 255]);
  await shot("52-studio-dark-en");
  await click("#btn-done");
  await wait("document.getElementById('ask-dialog').open");
  await shot("53-unsaved-dialog-dark-en");
  await answer('discard');
  await value("localStorage.setItem('karps.theme','light')");
}

try {
  await projectFlow();
  await handAndOutfits();
  await sharedOutfits();
  await paint3d();
  await functional();
  await controlChrome();
  await sliderDesign();
  await portraitScenarios();
  await discoveryScenarios();
  for (const theme of ["light", "dark"])
    for (const lang of ["en", "pt", "fr"]) {
      await layout(theme, lang, 1280, 720, lang !== "fr");
      await layout(theme, lang, 1440, 900, false);
    }
  for (const lang of ["es", "ja"]) await layout("light", lang, 1280, 720, false);
  check("no runtime exceptions", H.runtimeErrors.length === 0, H.runtimeErrors);
  const report = { checks: checks.length, screenshots: shots, fixture: "IPC fixture with synthetic textures and a synthetic mesh; not native runtime" };
  writeFileSync(resolve(evidence, "report.json"), JSON.stringify(report, null, 2));
  console.log(`UI OK: ${checks.length} checks, ${shots.length} screenshots in design/screens`);
} catch (e) {
  console.error("UI FAIL after", checks.length, "checks:", e.message);
  console.error(e.stack);
  try {
    console.error("failure screenshot:", await H.shot(resolve(H.scratch, "fail.png")));
  } catch {}
  process.exitCode = 1;
} finally {
  await H.close();
}
