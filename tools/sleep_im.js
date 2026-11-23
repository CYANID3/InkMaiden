/* ---------- InkMaiden: проект (исходная картинка и настройки), файлы для читалки ---------- */
const CTL=["thr","gap","outline","fillInside","invert","size","brush","lassoStroke","snapEdge"];
function ctlGet(){ const o={}; for(const k of CTL){ const e=$(k); if(e) o[k]= e.type==="checkbox" ? e.checked : +e.value; } return o; }
function ctlSet(o){ for(const k of CTL){ const e=$(k); if(e && o && k in o){ if(e.type==="checkbox") e.checked=o[k]; else e.value=o[k]; } } syncVals(); }
async function pack(u8){ const b=new Uint8Array(await new Response(new Blob([u8]).stream().pipeThrough(new CompressionStream("deflate"))).arrayBuffer());
  let t=""; for(let i=0;i<b.length;i+=32768) t+=String.fromCharCode.apply(null,b.subarray(i,i+32768)); return btoa(t); }
async function unpack(s){ const b=Uint8Array.from(atob(s),c=>c.charCodeAt(0)); return new Uint8Array(await new Response(new Blob([b]).stream().pipeThrough(new DecompressionStream("deflate"))).arrayBuffer()); }
window.__pbs={
  loadFile, has:()=>!!S.bbox,
  async state(){ return { v:1, ctl:ctlGet(), place:{...S.place}, flip:S.flip, corner:S.corner, w:S.w, h:S.h, ov:S.ov ? await pack(S.ov) : null }; },
  source(){ const im=S.img, c=document.createElement("canvas"); c.width=im.naturalWidth; c.height=im.naturalHeight; c.getContext("2d").drawImage(im,0,0); return c; },
  files(){ return buildFilesFor(MY[0]); },
  open(url, name, st){ loadURL(url, name, async()=>{
    ctlSet(st.ctl); S.flip=!!st.flip; $("flip").classList.toggle("on",S.flip);
    if(st.corner){ S.corner=st.corner; markCorner(S.corner); }
    if(st.ov && st.w===S.w && st.h===S.h) S.ov=await unpack(st.ov);
    processImage(); if(st.place) S.place={...st.place}; syncSizeSlider(); render(true);
    setStatus(`Открыта работа «${name}»`,"ok"); }); },
};
