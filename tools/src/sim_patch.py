# имитация экрана в обоях: подсветка выкл/вкл и ползунки, чтобы подогнать превью под свою читалку
SIM_JS = r"""
var SIM_DEF={v:{white:229,black:22,chroma:0,warm:6},i:{white:205,black:40,chroma:36,warm:8}};
var SIM_LIGHT={i:{white:226,black:34,chroma:46,warm:4},v:{white:240,black:24,chroma:0,warm:3}};
function loadSim(){var o=JSON.parse(JSON.stringify(SIM_DEF));try{var j=JSON.parse(localStorage.getItem("pbw-sim")||"null");if(j)["v","i"].forEach(function(k){if(j[k])for(var q in o[k])if(typeof j[k][q]==="number")o[k][q]=j[k][q];});}catch(e){}return o;}
var SIM=loadSim();
function saveSim(){try{localStorage.setItem("pbw-sim",JSON.stringify(SIM));}catch(e){}}
"""
wall = rep(wall, 'var DEF={b:0,c:0,s:0,g:0,', SIM_JS + 'var DEF={b:0,c:0,s:0,g:0,')
a = wall.index('function einkSimData(img,mode){'); b = wall.index('// simple (uncached) pipeline')
wall = wall[:a] + r"""function einkSimData(img,mode){
  // e-ink: 16 уровней на канал между «чёрным» и «бумагой» экрана; Kaleido — цвет вдвое реже (фильтр 2×2) и приглушён
  var W=img.width,H=img.height,d=img.data,i,xx,y,s=SIM[mode==="carta"?"v":"i"],w=s.white,t=s.warm;
  var P=[w+t*.25,w,w-t],K=[s.black,s.black,s.black+(mode==="carta"?2:4)];
  // упорядоченный дизеринг 4×4, как у прошивки при выводе картинок: плавные переходы вместо полос
  var BY=[0,8,2,10,12,4,14,6,3,11,1,9,15,7,13,5],dth=function(x,y){return (BY[(y&3)*4+(x&3)]-7.5)*1.06;};
  if(mode==="carta"){var LT=new Uint8ClampedArray(256*3);for(var v=0;v<256;v++){var g0=Math.round(v/17)/15;LT[v*3]=K[0]+(P[0]-K[0])*g0;LT[v*3+1]=K[1]+(P[1]-K[1])*g0;LT[v*3+2]=K[2]+(P[2]-K[2])*g0;}
    for(y=0;y<H;y++)for(xx=0;xx<W;xx++){i=(y*W+xx)*4;var gv=.299*d[i]+.587*d[i+1]+.114*d[i+2]+dth(xx,y);var g=(gv<0?0:gv>255?255:gv|0)*3;d[i]=LT[g];d[i+1]=LT[g+1];d[i+2]=LT[g+2];}}
  else{var hw=Math.ceil(W/2),Cc=new Float32Array(hw*Math.ceil(H/2)*3),Q=new Float32Array(16*3),cf=s.chroma/100,dkf=.06*Math.min(1.5,cf/.36);
    for(var qq=0;qq<16;qq++){Q[qq*3]=K[0]+(P[0]-K[0])*qq/15;Q[qq*3+1]=K[1]+(P[1]-K[1])*qq/15;Q[qq*3+2]=K[2]+(P[2]-K[2])*qq/15;}
    for(y=0;y<H;y++){var row=(y>>1)*hw;for(xx=0;xx<W;xx++){i=(y*W+xx)*4;var j=(row+(xx>>1))*3;Cc[j]+=d[i]*.25;Cc[j+1]+=d[i+1]*.25;Cc[j+2]+=d[i+2]*.25;}}
    for(y=0;y<H;y++){var row2=(y>>1)*hw;for(xx=0;xx<W;xx++){i=(y*W+xx)*4;var q=(row2+(xx>>1))*3,L=.299*d[i]+.587*d[i+1]+.114*d[i+2],cr=Cc[q],cg=Cc[q+1],cb=Cc[q+2],cl=.299*cr+.587*cg+.114*cb;
      var ch1=cr-cl,ch2=cg-cl,ch3=cb-cl,dk=(Math.abs(ch1)+Math.abs(ch2)+Math.abs(ch3))*dkf,e=dth(xx,y),a1=L-dk+ch1*cf+e,a2=L-dk+ch2*cf+e,a3=L-dk+ch3*cf+e;
      var r=Math.round((a1<0?0:a1>255?255:a1)/17),gg=Math.round((a2<0?0:a2>255?255:a2)/17),bb=Math.round((a3<0?0:a3>255?255:a3)/17);
      d[i]=Q[r*3];d[i+1]=Q[gg*3+1];d[i+2]=Q[bb*3+2];}}}
  return img;
}
""" + wall[b:]
wall = rep(wall, 'var src="var FAST=false;var DEV="+JSON.stringify(DEV)+";', 'var src="var FAST=false;var SIM="+JSON.stringify(SIM)+";var DEV="+JSON.stringify(DEV)+";')
wall = rep(wall, '"var o=m.st;for(var k in o)st[k]=o[k];', '"if(m.sim)SIM=m.sim;var o=m.st;for(var k in o)st[k]=o[k];')
wall = rep(wall, 'worker.postMessage({type:"job",id:id,ver:ver,', 'worker.postMessage({type:"job",sim:SIM,id:id,ver:ver,')
wall = rep(wall, 'if(!st.src)return;var key=[st.dev,st.srcId,', 'if(!st.src)return;var key=[JSON.stringify(SIM),st.dev,st.srcId,')
wall = rep(wall, '<button class="btn" id="lensBtn"', '<button class="btn" id="simBtn" title="Подогнать цвета превью под свою читалку"><svg viewBox="0 0 24 24"><rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h3"/></svg>Экран</button>\n        <button class="btn" id="lensBtn"')
SIM_CARD = """<div class="simcard" id="simCard" hidden>
          <div class="simhead"><b>Экран в превью</b><button class="rst" id="simReset">Сбросить</button></div>
          <div class="seg" id="simLight" style="display:flex;margin:6px 0 4px"><button data-l="off" style="flex:1">Подсветка выкл</button><button data-l="on" style="flex:1">Подсветка вкл</button></div>
          <label class="sl"><span>Бумага</span><input type="range" id="simWhite" min="150" max="250"><span id="simWhiteV"></span></label>
          <label class="sl"><span>Чёрный</span><input type="range" id="simBlack" min="0" max="80"><span id="simBlackV"></span></label>
          <label class="sl" id="simChromaRow"><span>Цвет</span><input type="range" id="simChroma" min="0" max="100"><span id="simChromaV"></span></label>
          <label class="sl"><span>Тёплый тон</span><input type="range" id="simWarm" min="0" max="20"><span id="simWarmV"></span></label>
          <p class="note">Положите читалку рядом с экраном и двигайте ползунки, пока картинки не совпадут. Настройка запомнится. На сам файл обоев она не влияет: это только превью.</p>
        </div>
        """
wall = rep(wall, '<div class="lens" id="lens">', SIM_CARD + '<div class="lens" id="lens">')
SIM_UI = r"""/* ---- настройка экрана в превью ---- */
function simKey(){return st.dev==="v"?"v":"i";}
function syncSimUI(){var s=SIM[simKey()];["white","black","chroma","warm"].forEach(function(k){var id="sim"+k[0].toUpperCase()+k.slice(1);$(id).value=s[k];$(id+"V").textContent=k==="chroma"?s[k]+"%":s[k];});
  $("simChromaRow").hidden=st.dev==="v";var L=SIM_LIGHT[simKey()],D=SIM_DEF[simKey()],eq=function(a,b){return ["white","black","chroma","warm"].every(function(k){return a[k]===b[k];});};
  [].forEach.call($("simLight").children,function(b){b.classList.toggle("on",b.dataset.l==="on"?eq(s,L):eq(s,D));});}
var simT=0;function simChanged(){saveSim();syncSimUI();clearTimeout(simT);simT=setTimeout(function(){thKey="";invalidate();},60);}
$("simBtn").addEventListener("click",function(){var c=$("simCard");c.hidden=!c.hidden;this.classList.toggle("on",!c.hidden);if(!c.hidden)syncSimUI();});
["white","black","chroma","warm"].forEach(function(k){$("sim"+k[0].toUpperCase()+k.slice(1)).addEventListener("input",function(){SIM[simKey()][k]=+this.value;simChanged();});});
$("simLight").addEventListener("click",function(e){var b=e.target.closest("button");if(!b)return;SIM[simKey()]=JSON.parse(JSON.stringify((b.dataset.l==="on"?SIM_LIGHT:SIM_DEF)[simKey()]));simChanged();});
$("simReset").addEventListener("click",function(){SIM[simKey()]=JSON.parse(JSON.stringify(SIM_DEF[simKey()]));simChanged();});
$("devSeg").addEventListener("click",function(){if(!$("simCard").hidden)setTimeout(syncSimUI,0);});
"""
wall = rep(wall, 'renderPanel();layoutAll();draw();\n})();', SIM_UI + 'renderPanel();layoutAll();draw();\n})();')
