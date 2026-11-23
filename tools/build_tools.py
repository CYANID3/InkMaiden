"""Собирает встроенные редакторы InkMaiden из исходников «Мастерской»: ui/tools/wallpaper.html и ui/tools/sleep.html.

Редакторы не переписываются целиком: поверх исходника накладываются точечные правки (как в build_studio.py).
- читалка одна — текущая в InkMaiden (parent.inkmaidenBridge), имитация экрана — из её профиля;
- обои: «Сохранить» и «На читалку» — в каталог «Мои работы» через раздел «Обои» (с проектом, чтобы открыть снова);
- иконка сна: шапка, выбор читалок и запись скрыты — сохраняет и ставит раздел «Иконка сна»; наружу отдаётся
  window.__pbs (файлы для читалки, настройки, исходная картинка, открыть работу; код — tools/sleep_im.js).
Запуск: python3 tools/build_tools.py
"""
import pathlib, re

T = pathlib.Path(__file__).parent
SRC = T / 'src'
OUT = T.parent / 'ui' / 'tools'


def rep(s, old, new, count=1):
    if old not in s:
        raise SystemExit(f'не найдено: {old[:90]!r}')
    return s.replace(old, new, count)


def head_add(s, extra):
    return rep(s, '</head>', extra + '\n</head>')


FONTS = '''<style>
@font-face{font-family:Onest;font-weight:400;src:url(../fonts/onest-cyrillic-400-normal.woff2) format("woff2");unicode-range:U+0400-04FF}
@font-face{font-family:Onest;font-weight:400;src:url(../fonts/onest-latin-400-normal.woff2) format("woff2");unicode-range:U+0000-00FF,U+2000-206F,U+2212}
@font-face{font-family:Onest;font-weight:500;src:url(../fonts/onest-cyrillic-500-normal.woff2) format("woff2");unicode-range:U+0400-04FF}
@font-face{font-family:Onest;font-weight:500;src:url(../fonts/onest-latin-500-normal.woff2) format("woff2");unicode-range:U+0000-00FF,U+2000-206F,U+2212}
@font-face{font-family:Onest;font-weight:600;src:url(../fonts/onest-cyrillic-600-normal.woff2) format("woff2");unicode-range:U+0400-04FF}
@font-face{font-family:Onest;font-weight:600;src:url(../fonts/onest-latin-600-normal.woff2) format("woff2");unicode-range:U+0000-00FF,U+2000-206F,U+2212}
</style>
<script>
var IM = window.parent && window.parent.inkmaidenBridge;
if (!IM) document.write('<p style="font:16px sans-serif;padding:20px">Этот редактор открывается внутри InkMaiden.</p>');
document.documentElement.dataset.theme = IM ? IM.theme() : 'light';
</script>'''

# ================================================================ иконка сна
sleep = (SRC / 'sleep.html').read_text(encoding='utf-8')
sleep = re.sub(r'<link rel="preconnect"[^>]*>\s*', '', sleep)
sleep = re.sub(r'<link href="https://fonts\.googleapis\.com[^>]*>\s*', '', sleep)
sleep = head_add(sleep, FONTS + '''<style>
/* внутри InkMaiden: шапку, выбор читалок и запись заменяет верхняя панель раздела; шрифт программы, тёмная тема */
:root{--serif:"Onest","Segoe UI",system-ui,sans-serif}
body{grid-template-rows:1fr auto auto}
body>header,[data-tab="readers"],#pane-readers{display:none!important}
/* журнал — во всплывающем окне InkMaiden после установки, здесь не нужен */
#logToggle,#logbox{display:none!important}
:root[data-theme="dark"]{--paper:#2B2B28;--panel:#32322F;--ink:#EEEEE7;--muted:#AEAEA4;--rule:#4A4A45;--olive:#C4C487;--olive-deep:#D6D69E;--olive-wash:#41412D;--graphite:#1E1E1C}
:root[data-theme="dark"] .btn,:root[data-theme="dark"] .chip,:root[data-theme="dark"] select,:root[data-theme="dark"] input:not([type=range]):not([type=checkbox]){background:#3A3A36;color:#EEEEE7;border-color:#4A4A45}
:root[data-theme="dark"] .btn.primary{background:#EEEEE7;color:#2B2B28;border-color:#EEEEE7}
:root[data-theme="dark"] #log{background:#242422;color:#D8D8CF}
:root[data-theme="dark"] #drop,:root[data-theme="dark"] .seg{background:#3A3A36}
:root[data-theme="dark"] .seg button.on,:root[data-theme="dark"] .btn.on{background:#EEEEE7;color:#2B2B28;border-color:#EEEEE7}
</style>''')
# одна читалка — текущая в InkMaiden
sleep = rep(sleep, re.search(r'const MODELS = \{.*?\n\};', sleep, re.S).group(0), 'const MODELS = IM.sleepModels();')
sleep = rep(sleep, 'let MY=loadMy();', 'let MY=Object.keys(MODELS);')
# запись на читалку, диски и метки делает InkMaiden (сохранение в каталог + установка с резервной копией)
sleep = rep(sleep, 'async function renderChips(){', 'async function renderChips(){ return;')
sleep = rep(sleep, 'setInterval(()=>{ if(document.visibilityState==="visible") renderChips(); }, 4000);', '')
# проект: исходная картинка и настройки, чтобы работу из каталога можно было открыть и править дальше
sleep = rep(sleep, 'function loadURL(url, name){', 'function loadURL(url, name, after){')
sleep = rep(sleep, '    S.src=g; S.w=w; S.h=h;', '    S.img=img; S.src=g; S.w=w; S.h=h;')
sleep = rep(sleep, '''+(k<1?`, для обработки уменьшена до ${iw}×${ih}`:""));
  };''', '''+(k<1?`, для обработки уменьшена до ${iw}×${ih}`:""));
    if(after) after();
  };''')
sleep = rep(sleep, '/* ---------- запуск ---------- */', (T / 'sleep_im.js').read_text(encoding='utf-8') + '\n/* ---------- запуск ---------- */')

# ================================================================ обои
wall = (SRC / 'wallpaper.html').read_text(encoding='utf-8')
WALL_THEME = '''<style>
/* оформка InkMaiden: бумага e-ink, графит, оливковый акцент (как в «Мастерской») */
:root{--bg:#ECECE6;--panel:#F7F7F3;--panel2:#FFFFFF;--line:#D6D6CC;--fg:#1B1B1A;--muted:#62625B;--accent:#55552A;--accent2:#55552A;--good:#4C6A2C;--warn:#9E3D2C}
:root[data-theme="dark"]{--bg:#2B2B28;--panel:#32322F;--panel2:#3A3A36;--line:#4A4A45;--fg:#EEEEE7;--muted:#AEAEA4;--accent:#D6D69E;--accent2:#C4C487}
body{font-family:"Onest","Segoe UI",system-ui,sans-serif}
.logo{display:none}
.ph{font:600 17px/1.3 "Onest",system-ui,sans-serif}
.btn{background:var(--panel2)}
.btn.primary{background:#55552A;border-color:#55552A;color:#fff}
.btn.on{border-color:#808040;color:var(--accent);background:var(--panel2)}
.seg,.mini{background:var(--bg)}
.seg button.on,.mini button.on{background:var(--fg);color:var(--bg)}
.seg button.on small{color:var(--muted)}
.stage{background:radial-gradient(circle at 50% 45%,#2b2b29,#151514)}
.drop{background:var(--bg)}
.pre,.layer,.chip,.fonts button,textarea,select,.expcard{background:var(--panel2)}
.pre.on,.layer.on{border-color:#808040}
.toast{background:#1B1B1A}
input[type=range]{accent-color:#808040}
.expcard .row2{display:flex;gap:8px;margin-top:8px}
.expcard .row2 .btn{flex:1}
.simcard{position:absolute;right:14px;bottom:14px;width:300px;background:var(--panel);color:var(--fg);border:1px solid var(--line);border-radius:12px;padding:12px 14px;z-index:6;box-shadow:0 10px 30px #0004}
.simcard[hidden]{display:none}
.simhead{display:flex;justify-content:space-between;align-items:center}
.simhead b{font:600 15px "Onest",sans-serif}
.simcard .rst{background:none;border:0;color:var(--accent);cursor:pointer;font-size:.8rem}
.simcard .sl{grid-template-columns:78px 1fr 40px}
#devSeg{display:none}
/* сохранение — кнопками InkMaiden внизу раздела */
#topExport{display:none!important}
.splitlbl{color:#fff}
.tilt.on{cursor:grab}
.tilt.grab{cursor:grabbing;transition:none}
</style>'''
wall = head_add(wall, FONTS + WALL_THEME)
# читалки из InkMaiden
wall = rep(wall, re.search(r'var DEV=\{\n  v:\{.*?\n\};', wall, re.S).group(0), 'var DEV=IM.wallDevices();var DK=Object.keys(DEV);function isG(k){return DEV[k].mode==="carta";}')
wall = rep(wall, '''      <button data-d="v" class="on">Verse<small>758×1024 · Ч/Б</small></button>
      <button data-d="i">InkPad Color 3<small>1404×1872 · цвет</small></button>
''', '')
wall = rep(wall, 'var st={dev:"v",', 'var st={dev:DK[0],')
wall = rep(wall, 'var EINK={v:', 'var EINK0={v:')
wall = rep(wall, 'var FONTS=[', 'var EINK={};DK.forEach(function(k){EINK[k]=EINK0[isG(k)?"v":"i"];});\nvar FONTS=[')
wall = rep(wall, 'var sat=k==="v"?', 'var sat=DEV[k].mode==="carta"?')
wall = rep(wall, 'if(k==="v"||S.s<=-100)quantGray(', 'if(DEV[k].mode==="carta"||S.s<=-100)quantGray(')
wall = rep(wall, "    if(k===\"v\"){for(var yy=18;", "    if(DEV[k].mode===\"carta\"){for(var yy=18;")
wall = rep(wall, 'isV=st.dev==="v";', 'isV=isG(st.dev);')
wall = rep(wall, "(isV?\"Verse\":\"InkPad\")", "D().name")
wall = rep(wall, "'<p class=\"note\">Verse всегда показывает картинку в оттенках серого", "'<p class=\"note\">'+esc(D().name)+' показывает картинку в оттенках серого")
wall = rep(wall, "'<p class=\"note\">На Verse любой цвет станет оттенком серого.</p>'", "'<p class=\"note\">На '+esc(D().name)+' любой цвет станет оттенком серого.</p>'")
# экспорт: по карточке на каждую читалку — «Сохранить» и «На читалку»
wall = rep(wall, '''      '<div class="sec">'+["v","i"].map(function(k){var d=DEV[k];return '<div class="expcard"><div class="t"><b>'+d.name+'</b><span>'+d.W+'×'+d.H+' · '+(k==="v"?"8 бит, серый":"24 бит, цвет")+'</span></div><button class="btn'+(k===st.dev?" primary":"")+'" style="width:100%" data-dl="'+k+'"'+(st.src?"":" disabled")+'>Скачать для '+d.name+'</button></div>';}).join("")+
      '<button class="btn" style="width:100%" data-dl="both"'+(st.src?"":" disabled")+'>Скачать оба файла</button>'+''',
'''      '<div class="sec">'+DK.map(function(k){var d=DEV[k],on=IM.connected(k);return '<div class="expcard"><div class="t"><b>'+esc(d.name)+'</b><span>'+d.W+'×'+d.H+' · '+(isG(k)?"8 бит, серый":"24 бит, цвет")+'</span></div><div class="row2"><button class="btn primary" data-dl="'+k+'"'+(st.src?"":" disabled")+'>Сохранить</button><button class="btn" data-up="'+k+'"'+(st.src&&on?"":" disabled")+' title="'+(on?"Папка на читалке: "+esc(IM.folder("readerWallpapers")):"Читалка не подключена")+'">На читалку</button></div></div>';}).join("")+
      '<p class="note">Работа сохраняется в «Мои работы» — папка '+esc(IM.folder("wallpapers"))+'. Папки меняются в «Настройках».</p>'+''')
wall = rep(wall, '[data-fmt],[data-dl]");if(!b)return;', '[data-fmt],[data-dl],[data-up]");if(!b)return;')
wall = rep(wall, '  if(b.dataset.dl){doExport(b.dataset.dl);return;}', '  if(b.dataset.dl){doExport(b.dataset.dl);return;}\n  if(b.dataset.up){doExport(b.dataset.up,true);return;}')
wall = rep(wall, '''function save(blob,name){var a=document.createElement("a"),url=URL.createObjectURL(blob);a.href=url;a.download=name;document.body.appendChild(a);a.click();a.remove();setTimeout(function(){URL.revokeObjectURL(url);},4000);}''',
               '''var UPTO=null;function save(blob,name){var k=UPTO;UPTO=null;IM.saveWall(name,blob,!!k).catch(function(e){toast(String(e.message||e));});}''')
wall = rep(wall, 'function finish(c){if(st.fmt==="png"){c.toBlob(function(b){save(b,name);if(done)done(name);},"image/png");}else{save(makeBMP(c,k==="v"),name);if(done)done(name);}}',
               'function finish(c){var up=exportOne.up;if(st.fmt==="png"){c.toBlob(function(b){UPTO=up;save(b,name);if(done)done(name);},"image/png");}else{UPTO=up;save(makeBMP(c,isG(k)),name);if(done)done(name);}}')
wall = rep(wall, '''function doExport(which){if(!st.src)return;toast("Готовлю файл…");
  if(which==="both"){exportOne("v",function(){setTimeout(function(){exportOne("i",function(){toast("Скачано 2 файла");});},400);});}
  else exportOne(which,function(n){toast("Готово: "+n);});}''',
'''function doExport(which,up){if(!st.src)return;toast("Готовлю файл…");
  if(which==="both"){var ks=DK.slice();(function next(){var k=ks.shift();if(!k)return;exportOne.up=null;exportOne(k,function(){setTimeout(next,300);});})();}
  else{exportOne.up=up?which:null;exportOne(which,function(){});}}''')
wall = rep(wall, '''$("topExport").innerHTML='<svg viewBox="0 0 24 24"><path d="M12 4v12M7 11l5 5 5-5M5 20h14"/></svg>Скачать BMP';''',
               '''$("topExport").innerHTML='<svg viewBox="0 0 24 24"><path d="M12 4v12M7 11l5 5 5-5M5 20h14"/></svg>Сохранить BMP';''')
wall = rep(wall, '<path d="M12 4v12M7 11l5 5 5-5M5 20h14"/></svg>Скачать BMP</button>', '<path d="M12 4v12M7 11l5 5 5-5M5 20h14"/></svg>Сохранить BMP</button>')
wall = rep(wall, '<path d="M12 4v12M7 11l5 5 5-5M5 20h14"/></svg>Скачать</button>', '<path d="M12 4v12M7 11l5 5 5-5M5 20h14"/></svg>Сохранить</button>')
wall = rep(wall, 'title="Скачать BMP или PNG"', 'title="Сохранить BMP или PNG"')
wall = rep(wall, "h='<div class=\"ph\">Скачать</div>", "h='<div class=\"ph\">Сохранить</div>")
wall = rep(wall, '''<ol class="steps"><li>Подключите ридер к компьютеру по USB.</li><li>Откройте скрытую папку <code>system/logo</code> (подпапки <code>offlogo</code> — при выключении, <code>bootlogo</code> — при загрузке). Windows: включите показ скрытых файлов. Mac: <b>Cmd + Shift + .</b> в Finder.</li><li>Скопируйте BMP и отключите ридер.</li><li>''',
  '''<ol class="steps"><li>Нажмите «В загрузки» внизу — обои добавятся в общий список переноса.</li><li>Файл ляжет в <code>'+esc(IM.folder("readerWallpapers"))+'</code> на читалке (папка меняется в «Настройках»).</li><li>Выберите его на читалке как заставку. ''')
wall = rep(wall, '<h4>Куда положить на ридере</h4>', '<h4>Как поставить на читалку</h4>')
# имитация экрана (подсветка, ползунки) — как в «Мастерской», значения по умолчанию из профилей читалок
exec((SRC / 'sim_patch.py').read_text(encoding='utf-8'))
wall = rep(wall, 'var SIM_DEF={v:{white:229,black:22,chroma:0,warm:6},i:{white:205,black:40,chroma:36,warm:8}};',
               'var SIM_DEF=(function(){var o={v:{white:229,black:22,chroma:0,warm:6},i:{white:205,black:40,chroma:36,warm:8}};DK.forEach(function(k){var s=DEV[k].sim;if(s)o[isG(k)?"v":"i"]={white:s.paper,black:s.ink,chroma:s.chroma,warm:s.warm};});return o;})();')
# 3D: повернуть читалку мышью
wall = rep(wall, '$("tiltBtn").addEventListener("click",function(){st.tilt=!st.tilt;this.classList.toggle("on",st.tilt);paintDevice();});',
           '$("tiltBtn").addEventListener("click",function(){st.tilt=!st.tilt;this.classList.toggle("on",st.tilt);paintDevice();});\n' + (T / 'wall_tilt.js').read_text(encoding='utf-8'))
wall = rep(wall, 'function simKey(){return st.dev==="v"?"v":"i";}', 'function simKey(){return isG(st.dev)?"v":"i";}')
wall = rep(wall, '$("simChromaRow").hidden=st.dev==="v";', '$("simChromaRow").hidden=isG(st.dev);')
wall = rep(wall, 'window.__pbw={st:st,', 'window.__pbw={loadFile:loadFile,setSource:setSource,snap:snap,restore:restore,orig:function(){return st.orig;},fmt:function(){return st.fmt;},exportTo:function(up){doExport(DK[0],up);},st:st,')
# кнопки читалок в шапке — из InkMaiden; при изменении списка читалок редактор перезагружается
wall = rep(wall, '\nrenderPanel();layoutAll();draw();\n})();', '''
(function(){var seg=$("devSeg");seg.innerHTML=DK.map(function(k){var d=DEV[k];return '<button data-d="'+k+'" class="'+(k===st.dev?"on":"")+'">'+esc(d.name)+'<small>'+d.W+'×'+d.H+' · '+(isG(k)?"Ч/Б":"цвет")+'</small></button>';}).join("");
  var sig=JSON.stringify(DK);IM.onDevices(function(){if(JSON.stringify(Object.keys(IM.wallDevices()))!==sig)location.reload();else if(st.tool==="export")renderPanel();});})();
renderPanel();layoutAll();draw();
})();''')

OUT.mkdir(parents=True, exist_ok=True)
(OUT / 'sleep.html').write_text(sleep, encoding='utf-8')
(OUT / 'wallpaper.html').write_text(wall, encoding='utf-8')
print('ok', len(sleep), len(wall))
