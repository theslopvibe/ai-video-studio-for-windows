// AI tools for the Image editor in AI Video Studio. miniPaint loads every file in modules/ as a menu target ("ai/ai.<method>").
// The page cannot reach ComfyUI itself: it asks the host app (parent window) with postMessage and inserts the answer as a layer.
import app from './../../app.js';
import config from './../../config.js';
import Dialog_class from './../../libs/popup.js';

const WAIT_MS = 15 * 60 * 1000;
let counter = 0;

class AI_class {
	constructor() {
		this.POP = new Dialog_class();
	}

	ask(op, payload) {
		return new Promise((resolve, reject) => {
			const id = 'ai' + (++counter);
			const timer = setTimeout(() => { window.removeEventListener('message', on); reject(new Error('Timed out')); }, WAIT_MS);
			const on = (ev) => {
				if (ev.source !== window.parent || !ev.data || ev.data.avs !== 'ai-result' || ev.data.id !== id) return;
				clearTimeout(timer);
				window.removeEventListener('message', on);
				ev.data.error ? reject(new Error(ev.data.error)) : resolve(ev.data.image);
			};
			window.addEventListener('message', on);
			window.parent.postMessage({ avs: 'ai', id, op, ...payload }, '*');
		});
	}

	insert(dataUrl, name) {
		const img = new Image();
		img.onload = () => {
			const layer = { name, type: 'image', link: img, width: img.width, height: img.height, width_original: img.width, height_original: img.height };
			const actions = [new app.Actions.Insert_layer_action(layer)];
			if (config.layers.length <= 1 && !config.layers[0]?.type) actions.push(new app.Actions.Autoresize_canvas_action(img.width, img.height, null, true, true));
			app.State.do_action(new app.Actions.Bundle_action('ai_result', name, actions));
			config.need_render = true;
		};
		img.src = dataUrl;
	}

	async run(op, payload, name) {
		alertify.message('AI: working… this can take a minute. You can keep editing.');
		try {
			this.insert(await this.ask(op, payload), name);
			alertify.success('AI: done, added as a new layer.');
		} catch (x) {
			alertify.error('AI: ' + x.message);
		}
	}

	generate() {
		this.POP.show({
			title: 'Generate image into a layer',
			params: [{ name: 'prompt', title: 'Describe the image:', value: '' }],
			on_finish: (p) => { if (p.prompt.trim()) this.run('generate', { prompt: p.prompt.trim(), width: config.WIDTH || 1024, height: config.HEIGHT || 1024 }, 'AI image'); },
		});
	}
}

export default AI_class;
