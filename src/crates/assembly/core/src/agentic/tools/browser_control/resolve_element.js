(function(selector) {
    let crossOriginFrames = 0;
    function findIn(root) {
        const direct = root.querySelector(selector);
        if (direct) return direct;
        for (const node of root.querySelectorAll('*')) {
            if (node.shadowRoot) {
                const hit = findIn(node.shadowRoot);
                if (hit) return hit;
            }
            if (node.tagName === 'IFRAME' || node.tagName === 'FRAME') {
                let doc = null;
                try { doc = node.contentDocument; } catch (_) {}
                if (doc) {
                    const hit = findIn(doc);
                    if (hit) return hit;
                } else {
                    crossOriginFrames++;
                }
            }
        }
        return null;
    }
    const el = findIn(document);
    if (!el) {
        throw new Error('Element not found: ' + selector + ' — take a fresh snapshot or check shadow/iframe scope'
            + (crossOriginFrames ? ' (page contains ' + crossOriginFrames + ' cross-origin iframe(s) whose contents cannot be inspected)' : ''));
    }
    return el;
})
