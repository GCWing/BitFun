/** Trusted wrapper and untrusted H5 are separate browsing contexts. */
export class MiniAppDocument {
  static build(html: string, locale: string): string {
    const source = JSON.stringify(html).replace(/</g, '\\u003c');
    const language = JSON.stringify(locale).replace(/</g, '\\u003c');
    return `<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline' https: blob:; style-src 'unsafe-inline' https:; img-src https: data: blob:; font-src https: data:; connect-src https:; frame-src blob:">
<style>html,body{margin:0;height:100%;overflow:hidden}iframe{width:100%;height:100%;border:0;display:block}</style>
</head><body><iframe sandbox="allow-scripts" title="MiniApp"></iframe><script>
(()=>{
const frame=document.querySelector('iframe');
const pending=new Map();
window.__miniappReply=(reply)=>{if(!pending.has(reply.id))return;clearTimeout(pending.get(reply.id));pending.delete(reply.id);frame.contentWindow.postMessage(reply,'*');};
window.addEventListener('message',event=>{
 if(event.source!==frame.contentWindow)return;
 const data=event.data;
 if(!data||typeof data!=='object')return;
 if(data.method==='openbitfun/request-locale'){
   frame.contentWindow.postMessage({type:'openbitfun:event',event:'localeChange',payload:{locale:${language}}},'*');return;
 }
 if(data.method==='openbitfun/request-appearance')return;
 if(typeof data.id!=='string'||typeof data.method!=='string'||pending.has(data.id))return;
 const replyError=message=>window.__miniappReply({jsonrpc:'2.0',id:data.id,error:{message}});
 pending.set(data.id,setTimeout(()=>replyError('The desktop did not respond. The operation may still finish; it was not retried.'),65000));
 if(data.method!=='worker.call'){replyError('This capability is not supported in the mobile MiniApp preview: '+data.method);return;}
 try{miniappNative.request(JSON.stringify({id:data.id,method:data.method,params:data.params}));}
 catch(error){replyError(String(error));}
});
const url=URL.createObjectURL(new Blob([${source}],{type:'text/html'}));
frame.src=url;
frame.addEventListener('load',()=>frame.contentWindow.postMessage({type:'openbitfun:event',event:'activate',payload:{}},'*'));
})();
</script></body></html>`;
  }
}
