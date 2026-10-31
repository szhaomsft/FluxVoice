const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');

function harness({ delayedState = false, failStart = false } = {}) {
  const slots = [];
  const effects = [];
  const listeners = new Map();
  let cursor = 0;
  let state = 'idle';
  let starting = false;
  let starts = 0;
  let stops = 0;
  let registrations = 0;
  let resolveStart;
  const gate = new Promise(resolve => { resolveStart = resolve; });
  const react = {
    useRef(value) {
      const index = cursor++;
      return slots[index] ||= { current: value };
    },
    useState(value) {
      const index = cursor++;
      slots[index] ||= { value };
      return [slots[index].value, value => { slots[index].value = value; }];
    },
    useCallback(callback) { cursor++; return callback; },
    useEffect(callback, deps) {
      const index = cursor++;
      const previous = slots[index];
      if (!previous || deps.some((dep, i) => dep !== previous.deps[i])) {
        effects.push(() => {
          previous?.cleanup?.();
          slots[index] = { deps, cleanup: callback() };
        });
      }
    },
  };
  const setError = () => {};
  const useAppStore = () => ({ recordingState: state, setError });
  useAppStore.getState = () => ({ recordingState: state });
  const dependencies = {
    react,
    'react/jsx-runtime': { jsx: () => null, jsxs: () => null },
    '@tauri-apps/api/core': {
      invoke: async name => name === 'get_config'
        ? { features: { postProcessingMode: 'none', translateTargetLanguage: 'English' } }
        : false,
    },
    '@tauri-apps/api/app': { getVersion: async () => 'test' },
    '@tauri-apps/api/window': {
      getCurrentWindow: () => ({ onFocusChanged: async () => () => {} }),
    },
    '@tauri-apps/api/event': {
      listen: async (name, callback) => {
        registrations++;
        listeners.set(name, callback);
        return () => listeners.delete(name);
      },
    },
    'lucide-react': {},
    '../../store/appStore': { useAppStore },
    '../../hooks/useAudioRecording': {
      useAudioRecording: () => ({
        startRecording: async () => {
          starts++;
          starting = true;
          if (!delayedState) state = 'recording';
          await gate;
          state = failStart ? 'idle' : 'recording';
          starting = false;
        },
        stopRecording: async () => {
          assert.equal(starting, false, 'Stop must wait for startup lock release');
          if (state !== 'recording') return;
          stops++;
          state = 'processing';
        },
      }),
    },
    '../../types/config': {},
  };
  for (const component of ['Waveform', 'IdleAnimation', 'ProcessingAnimation', 'StatusIndicator']) {
    dependencies[`./${component}`] = {};
  }
  const filename = path.resolve(__dirname, '..', 'src', 'components', 'FloatingWindow', 'FloatingWindow.tsx');
  const compiled = ts.transpileModule(fs.readFileSync(filename, 'utf8'), {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true,
    },
    fileName: filename,
  }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, {
    module, exports: module.exports, Date,
    console: { log() {}, error() {} },
    require(name) {
      assert.ok(name in dependencies, `Unexpected dependency: ${name}`);
      return dependencies[name];
    },
  }, { filename });
  function render() {
    cursor = 0;
    module.exports.FloatingWindow();
    while (effects.length) effects.shift()();
  }
  render();
  return {
    render, resolveStart,
    press: () => listeners.get('hotkey-pressed')(),
    release: () => listeners.get('hotkey-released')(),
    starts: () => starts, stops: () => stops,
    state: () => state, registrations: () => registrations,
  };
}

for (const delayedState of [false, true]) {
  test(`release during startup is retained with ${delayedState ? 'idle' : 'recording'} state`, async () => {
    const h = harness({ delayedState });
    const press = h.press();
    h.render();
    await h.release();
    await h.release();
    assert.equal(h.stops(), 0);
    h.resolveStart();
    await press;
    assert.equal(h.stops(), 1);
    assert.equal(h.state(), 'processing');
    await h.release();
    assert.equal(h.stops(), 1);
    assert.equal(h.registrations(), 2);
  });
}

test('release after startup reads current store without waiting for a render', async () => {
  const h = harness();
  const press = h.press();
  h.resolveStart();
  await press;
  await h.release();
  assert.equal(h.stops(), 1);
  assert.equal(h.state(), 'processing');
});

test('failed startup discards queued release without attempting a stop', async () => {
  const h = harness({ failStart: true });
  const press = h.press();
  await h.release();
  h.resolveStart();
  await press;
  assert.equal(h.stops(), 0);
  assert.equal(h.state(), 'idle');
});

test('rerenders replace recording callbacks without replacing hotkey listeners', async () => {
  const h = harness();
  h.render();
  h.render();
  await Promise.resolve();
  assert.equal(h.registrations(), 2);
  const press = h.press();
  h.resolveStart();
  await press;
  await h.release();
  assert.equal(h.starts(), 1);
  assert.equal(h.stops(), 1);
});
