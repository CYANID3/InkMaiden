/* InkMaiden: в режиме 3D читалку можно повернуть мышью; двойной щелчок — обратно в исходный наклон */
(function(){var t=$("tilt"),rx=16,ry=-20,rz=4,drag=null;
  function apply(){t.style.transform=st.tilt?"rotateX("+rx.toFixed(1)+"deg) rotateY("+ry.toFixed(1)+"deg) rotateZ("+rz+"deg)":"";}
  t.addEventListener("pointerdown",function(e){if(!st.tilt||e.button!==0)return;drag={x:e.clientX,y:e.clientY,rx:rx,ry:ry};t.setPointerCapture(e.pointerId);t.classList.add("grab");e.preventDefault();e.stopPropagation();});
  t.addEventListener("pointermove",function(e){if(!drag)return;ry=Math.max(-75,Math.min(75,drag.ry+(e.clientX-drag.x)*.4));rx=Math.max(-65,Math.min(65,drag.rx-(e.clientY-drag.y)*.4));apply();});
  function end(){if(!drag)return;drag=null;t.classList.remove("grab");}
  t.addEventListener("pointerup",end);t.addEventListener("pointercancel",end);
  t.addEventListener("dblclick",function(){if(!st.tilt)return;rx=16;ry=-20;apply();toast("Наклон как был");});
  $("tiltBtn").addEventListener("click",apply);
  $("tiltBtn").title="Наклон в 3D: потяните читалку мышью, двойной щелчок — вернуть";
})();
