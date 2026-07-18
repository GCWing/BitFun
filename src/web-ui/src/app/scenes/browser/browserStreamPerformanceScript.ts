/**
 * Injected into browser webviews to reduce composition cost for realtime
 * canvas/video preview pages. It is intentionally conservative and only
 * activates for pages that look like BitFun's Harmony device preview stream.
 */
export const STREAM_RENDER_OPTIMIZATION_SCRIPT = /* js */ `(function(){
  if(window.__bitfun_stream_render_optimized){return;}

  // #region agent log
  function installStreamDiagnostics(){
    if(location.hostname!=='127.0.0.1'||location.port!=='41953'||window.__bitfun_stream_diagnostics){return;}
    window.__bitfun_stream_diagnostics=true;

    var endpoint='http://127.0.0.1:7469/log';
    var drawCount=0;
    var drawDuration=0;
    var drawMax=0;
    var rafCount=0;
    var lastReport=performance.now();
    var canvasPrototype=window.CanvasRenderingContext2D&&window.CanvasRenderingContext2D.prototype;
    var originalDrawImage=canvasPrototype&&canvasPrototype.drawImage;

    function post(message,data){
      void fetch(endpoint,{
        method:'POST',
        headers:{'Content-Type':'application/json'},
        body:JSON.stringify({
          hypothesis:'A/B/C',
          location:'browserStreamPerformanceScript.streamDiagnostics',
          message:message,
          data:data,
          timestamp:new Date().toISOString()
        })
      }).catch(function(){});
    }

    function gpuInfo(){
      try{
        var probe=document.createElement('canvas');
        var gl=probe.getContext('webgl')||probe.getContext('experimental-webgl');
        var extension=gl&&gl.getExtension('WEBGL_debug_renderer_info');
        return {
          vendor:extension?gl.getParameter(extension.UNMASKED_VENDOR_WEBGL):null,
          renderer:extension?gl.getParameter(extension.UNMASKED_RENDERER_WEBGL):null
        };
      }catch(error){
        return {error:String(error)};
      }
    }

    if(canvasPrototype&&originalDrawImage){
      canvasPrototype.drawImage=function(){
        var started=performance.now();
        try{return originalDrawImage.apply(this,arguments);}
        finally{
          var elapsed=performance.now()-started;
          drawCount+=1;
          drawDuration+=elapsed;
          drawMax=Math.max(drawMax,elapsed);
        }
      };
    }

    function sampleRaf(){
      rafCount+=1;
      requestAnimationFrame(sampleRaf);
    }
    requestAnimationFrame(sampleRaf);

    post('stream diagnostics initialized',{
      userAgent:navigator.userAgent,
      hardwareConcurrency:navigator.hardwareConcurrency,
      deviceMemory:navigator.deviceMemory||null,
      visibility:document.visibilityState,
      devicePixelRatio:window.devicePixelRatio,
      gpu:gpuInfo(),
      videoDecoder:typeof window.VideoDecoder,
      screen:{width:screen.width,height:screen.height,colorDepth:screen.colorDepth}
    });

    setInterval(function(){
      var now=performance.now();
      var span=Math.max(1,now-lastReport);
      var canvas=document.getElementById('screen');
      post('stream render interval',{
        spanMs:Math.round(span),
        drawFps:Number((drawCount*1000/span).toFixed(2)),
        drawAvgMs:drawCount?Number((drawDuration/drawCount).toFixed(3)):null,
        drawMaxMs:Number(drawMax.toFixed(3)),
        rafFps:Number((rafCount*1000/span).toFixed(2)),
        visibility:document.visibilityState,
        hasFocus:document.hasFocus(),
        canvas:canvas?{
          width:canvas.width,
          height:canvas.height,
          cssWidth:Math.round(canvas.getBoundingClientRect().width),
          cssHeight:Math.round(canvas.getBoundingClientRect().height)
        }:null
      });
      drawCount=0;
      drawDuration=0;
      drawMax=0;
      rafCount=0;
      lastReport=now;
    },1000);
  }
  // #endregion

  function looksLikeDeviceStream(){
    if(document.getElementById('screen')&&document.getElementById('stage')){return true;}
    if(document.title&&/harmony.*preview|device.*preview/i.test(document.title)){return true;}
    return false;
  }

  function optimize(){
    if(!looksLikeDeviceStream()){return;}
    window.__bitfun_stream_render_optimized=true;
    installStreamDiagnostics();

    var style=document.getElementById('bitfun-stream-render-optimization');
    if(!style){
      style=document.createElement('style');
      style.id='bitfun-stream-render-optimization';
      style.textContent=[
        'header{backdrop-filter:none!important;-webkit-backdrop-filter:none!important;box-shadow:none!important;}',
        '#screenFrame{box-shadow:none!important;}',
        '#screen{box-shadow:none!important;outline:none!important;transform:translateZ(0);will-change:transform;contain:paint;}'
      ].join('\\n');
      document.documentElement.appendChild(style);
    }

    var screen=document.getElementById('screen');
    if(screen){
      screen.style.transform='translateZ(0)';
      screen.style.willChange='transform';
      screen.style.contain='paint';
    }
  }

  optimize();
  if(document.readyState==='loading'){
    document.addEventListener('DOMContentLoaded',optimize,{once:true});
  }
  window.addEventListener('load',optimize,{once:true});
})()`;
