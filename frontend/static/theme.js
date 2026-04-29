const COLOR_INPUT_PREFIX = 'c-';
const COLOR_STORAGE_PREFIX = '--color-';
const ACTIVE_PRESET_STORAGE_KEY = 'snippies-active-theme-preset';
const PRESETS_URL = '/static/theme-presets.jsonl';
const SAVE_PRESET_URL = '/api/theme-presets';
const COLOR_VALUE_PATTERN = /^#[0-9a-f]{6}$/i;
const PRESET_NAME_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;

const themePresets = new Map();

function colorNameFromInputId(inputId) {
    return `${COLOR_STORAGE_PREFIX}${inputId.substring(COLOR_INPUT_PREFIX.length)}`;
}

function getColorInputs() {
    return [...document.querySelectorAll(`input[type="color"]`)];
}

function applyColor(inputId, color, persist) {
    const input = document.getElementById(inputId);

    if (!input || !COLOR_VALUE_PATTERN.test(color)) {
        return;
    }

    const colorName = colorNameFromInputId(inputId);
    input.value = color;
    document.documentElement.style.setProperty(colorName, color);

    if (persist) {
        localStorage.setItem(colorName, color);
    }
}

function getCurrentThemeColors() {
    return Object.fromEntries(getColorInputs().map(input => [input.id, input.value]));
}

function createThemePresetLine(name) {
    return JSON.stringify({ name, colors: getCurrentThemeColors() });
}

function setSaveStatus(message) {
    const status = document.getElementById('preset-save-status');
    status.textContent = message;
}

function setActivePreset(presetName) {
    document.querySelectorAll('.preset-swatch').forEach(button => {
        button.classList.toggle('active', button.dataset.preset === presetName);
    });

    if (presetName) {
        localStorage.setItem(ACTIVE_PRESET_STORAGE_KEY, presetName);
    } else {
        localStorage.removeItem(ACTIVE_PRESET_STORAGE_KEY);
    }
}

function applyPreset(presetName) {
    const preset = themePresets.get(presetName);

    if (!preset) {
        return;
    }

    for (const [inputId, color] of Object.entries(preset.colors)) {
        applyColor(inputId, color, true);
    }

    setActivePreset(presetName);
}

function normalizePreset(rawPreset) {
    if (!rawPreset || typeof rawPreset.name !== 'string' || !rawPreset.colors) {
        return null;
    }

    const colors = {};

    for (const [inputId, color] of Object.entries(rawPreset.colors)) {
        if (inputId.startsWith(COLOR_INPUT_PREFIX) && COLOR_VALUE_PATTERN.test(color)) {
            colors[inputId] = color;
        }
    }

    if (!Object.keys(colors).length) {
        return null;
    }

    return {
        name: rawPreset.name,
        swatch: COLOR_VALUE_PATTERN.test(rawPreset.swatch) ? rawPreset.swatch : colors['c-bg'],
        colors,
    };
}

function parsePresetLines(contents) {
    return contents
        .split(/\r?\n/)
        .map(line => line.trim())
        .filter(Boolean)
        .map(line => {
            try {
                return normalizePreset(JSON.parse(line));
            } catch {
                return null;
            }
        })
        .filter(Boolean);
}

function renderPresetSwatches(presets) {
    const swatches = document.getElementById('preset-swatches');

    swatches.replaceChildren();
    themePresets.clear();

    for (const preset of presets) {
        const button = document.createElement('button');
        button.className = 'preset-swatch';
        button.dataset.preset = preset.name;
        button.title = preset.name;
        button.type = 'button';
        button.style.backgroundColor = preset.swatch;
        swatches.appendChild(button);
        themePresets.set(preset.name, preset);
    }

    setActivePreset(localStorage.getItem(ACTIVE_PRESET_STORAGE_KEY));
}

function upsertPresetSwatch(preset) {
    const swatches = document.getElementById('preset-swatches');
    let button = [...swatches.querySelectorAll('.preset-swatch')]
        .find(swatch => swatch.dataset.preset === preset.name);

    themePresets.set(preset.name, preset);

    if (!button) {
        button = document.createElement('button');
        button.className = 'preset-swatch';
        button.dataset.preset = preset.name;
        button.type = 'button';
        swatches.appendChild(button);
    }

    button.title = preset.name;
    button.style.backgroundColor = preset.swatch;
}

async function loadPresetSwatches() {
    const response = await fetch(PRESETS_URL);

    if (!response.ok) {
        return;
    }

    renderPresetSwatches(parsePresetLines(await response.text()));
}

async function saveCurrentPreset() {
    const nameInput = document.getElementById('preset-name');
    const saveButton = document.getElementById('save-preset');
    const name = nameInput.value.trim();

    if (!name) {
        setSaveStatus('Enter a preset name.');
        return;
    }

    if (!PRESET_NAME_PATTERN.test(name)) {
        setSaveStatus('Use letters, numbers, hyphens, or underscores.');
        return;
    }

    const rawPreset = { name, colors: getCurrentThemeColors() };
    const preset = normalizePreset(rawPreset);

    if (!preset) {
        setSaveStatus('Current colors are not valid.');
        return;
    }

    saveButton.disabled = true;
    setSaveStatus('Saving...');

    try {
        const response = await fetch(SAVE_PRESET_URL, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            credentials: 'same-origin',
            body: JSON.stringify(rawPreset),
        });

        if (!response.ok) {
            setSaveStatus(await response.text() || 'Could not save preset.');
            return;
        }

        upsertPresetSwatch(preset);
        setActivePreset(preset.name);
        nameInput.value = '';
        setSaveStatus('Preset saved.');
    } catch {
        setSaveStatus('Could not save preset.');
    } finally {
        saveButton.disabled = false;
    }
}

function restoreSavedColors() {
    for (const input of getColorInputs()) {
        const colorName = colorNameFromInputId(input.id);
        const savedColor = localStorage.getItem(colorName);

        if (savedColor) {
            applyColor(input.id, savedColor, false);
        }

        input.addEventListener('input', e => {
            applyColor(e.target.id, e.target.value, true);
            setActivePreset(null);
        });
    }
}

function toggleDropdown() {
    const chevron = document.getElementById('chevron');
    const dropdown = document.getElementById('dropdown');
    const dropdownOpen = dropdown.classList.toggle('dropdown-open');

    chevron.textContent = dropdownOpen ? '▴' : '▾';
}

document.getElementById('preset-swatches').addEventListener('click', e => {
    const button = e.target.closest('.preset-swatch');

    if (!button) {
        return;
    }

    applyPreset(button.dataset.preset);
});

document.getElementById('save-preset').addEventListener('click', saveCurrentPreset);
document.getElementById('preset-name').addEventListener('keydown', e => {
    if (e.key === 'Enter') {
        e.preventDefault();
        saveCurrentPreset();
    }
});

restoreSavedColors();
loadPresetSwatches();

window.toggleDropdown = toggleDropdown;
window.applyPreset = applyPreset;
window.createThemePresetLine = createThemePresetLine;
