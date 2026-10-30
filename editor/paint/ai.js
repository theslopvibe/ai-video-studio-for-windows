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

	// the active layer as PNG (only that layer, with its transparency)
	layer_png() {
		const layer = app.Layers.get_layer();
		if (!layer || !layer.type) return null;
		const canvas = document.createElement('canvas');
		canvas.width = config.WIDTH;
		canvas.height = config.HEIGHT;
		app.Layers.convert_layers_to_canvas(canvas.getContext('2d'), layer.id, false);
		return { layer, png: canvas.toDataURL('image/png') };
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

	// waits for one click on the picture and returns it as "x,y" in image pixels
	pick_point(message) {
		return new Promise((resolve) => {
			const canvas = document.getElementById('canvas_minipaint');
			alertify.message(message, 0);
			const done = (value) => {
				canvas.removeEventListener('mousedown', onDown, true);
				window.removeEventListener('keydown', onKey, true);
				alertify.dismissAll();
				resolve(value);
			};
			const onDown = (ev) => {
				ev.preventDefault();
				ev.stopPropagation();
				const r = canvas.getBoundingClientRect();
				const x = Math.round(((ev.clientX - r.left) / r.width) * config.WIDTH);
				const y = Math.round(((ev.clientY - r.top) / r.height) * config.HEIGHT);
				done(x >= 0 && y >= 0 && x < config.WIDTH && y < config.HEIGHT ? x + ',' + y : null);
			};
			const onKey = (ev) => { if (ev.key === 'Escape') done(null); };
			canvas.addEventListener('mousedown', onDown, true);
			window.addEventListener('keydown', onKey, true);
		});
	}

	async with_point(op, message, name) {
		const l = this.layer_png();
		if (!l) return alertify.error('Select a layer with an image first.');
		const points = await this.pick_point(message);
		if (!points) return alertify.warning('Cancelled.');
		this.run(op, { image: l.png, points }, name);
	}

	cutout() { this.with_point('cutout', 'Click on the object you want to cut out (Esc to cancel)', 'Cut-out'); }

	erase() { this.with_point('erase', 'Click on the object you want to erase (Esc to cancel)', 'Erased'); }

	faces() { this.one_shot('faces', 'Faces restored'); }

	colorize() { this.one_shot('colorize', 'Colorized'); }

	depth() { this.one_shot('depth', 'Depth map'); }

	one_shot(op, name) {
		const l = this.layer_png();
		if (!l) return alertify.error('Select a layer with an image first.');
		this.run(op, { image: l.png }, name);
	}

	upscale() {
		const l = this.layer_png();
		if (!l) return alertify.error('Select a layer with an image first.');
		this.run('upscale', { image: l.png }, 'Upscaled ' + (l.layer.name || ''));
	}

	remove_background() {
		const l = this.layer_png();
		if (!l) return alertify.error('Select a layer with an image first.');
		this.run('remove_background', { image: l.png }, 'No background ' + (l.layer.name || ''));
	}

	edit() {
		const l = this.layer_png();
		if (!l) return alertify.error('Select a layer with an image first.');
		this.POP.show({
			title: 'Change the picture with words',
			params: [{ name: 'prompt', title: 'What should change?', value: '' }],
			on_finish: (p) => { if (p.prompt.trim()) this.run('edit', { image: l.png, prompt: p.prompt.trim() }, 'AI edit'); },
		});
	}
}

export default AI_class;
