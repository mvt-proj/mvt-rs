function initializePage() {
  let protocol = window.location.protocol;
  let host = window.location.hostname;
  let port = window.location.port;

  if (port === '') {
    port = protocol === 'https:' ? '443' : '80';
  }

  let servers = document.getElementsByClassName("server");
  for (let i = 0; i < servers.length; i++) {
    let serverAddress = `${protocol}//${host}`;
    if (port !== '80' && port !== '443') {
      serverAddress += `:${port}`;
    }
    servers[i].textContent = serverAddress;
  }
}

function copyToClipboard(id, buttonEl, dropdownId) {
  const urlElement = document.getElementById(id);
  const urlText = (urlElement.innerText || urlElement.textContent).trim();

  const finish = (success, err) => {
    if (!success) console.error("Error to copy: ", err);
    flashCopyFeedback(buttonEl, success);
    // Keep the menu open long enough for the feedback to be readable.
    setTimeout(() => { if (dropdownId) closeDropdown(dropdownId); }, 900);
  };

  if (window.isSecureContext && navigator.clipboard) {
    navigator.clipboard
      .writeText(urlText)
      .then(() => finish(true))
      .catch(() => fallbackCopy(urlText, () => finish(true), (err) => finish(false, err)));
  } else {
    fallbackCopy(urlText, () => finish(true), (err) => finish(false, err));
  }
}

// Clipboard API requires a secure context (HTTPS or localhost); this covers
// plain-HTTP deployments where navigator.clipboard is unavailable.
function fallbackCopy(text, onCopied, onFailed) {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  try {
    const ok = document.execCommand("copy");
    ok ? onCopied() : onFailed(new Error("execCommand('copy') returned false"));
  } catch (err) {
    onFailed(err);
  } finally {
    document.body.removeChild(textarea);
  }
}

function flashCopyFeedback(buttonEl, success) {
  if (!buttonEl) return;
  const label = buttonEl.querySelector("span:last-child");
  if (!label) return;
  const original = label.textContent;
  label.textContent = success ? "✓ Copiado" : "No se pudo copiar";
  setTimeout(() => {
    label.textContent = original;
  }, 1500);
}

function moveUp() {
    const select = document.getElementById("fields");
    for (let i = 1; i < select.options.length; i++) {
      const opt = select.options[i];
      if (opt.selected && !select.options[i - 1].selected) {
        select.insertBefore(opt, select.options[i - 1]);
      }
    }
  }

  function moveDown() {
    const select = document.getElementById("fields");
    for (let i = select.options.length - 2; i >= 0; i--) {
      const opt = select.options[i];
      if (opt.selected && !select.options[i + 1].selected) {
        select.insertBefore(select.options[i + 1], opt);
      }
    }
  }

let OPEN_DROPDOWN_ID = null;

function toggleDropdown(id) {
  const menu = document.getElementById(`dropdown-menu-${id}`);
  if (!menu) return;
  const isOpen = !menu.classList.contains('hidden');

  closeAllDropdowns();

  if (!isOpen) openDropdown(id);
}

function openDropdown(id) {
  const menu = document.getElementById(`dropdown-menu-${id}`);
  const btn = document.querySelector(`[data-dropdown="${id}"] button`);
  if (!menu || !btn) return;

  menu.classList.remove('hidden');

  menu.style.position = 'fixed';
  menu.style.maxHeight = '70vh';

  const btnRect = btn.getBoundingClientRect();
  const menuWidth = menu.offsetWidth;
  const menuHeight = menu.offsetHeight;

  const gapY = -5;
  const gapX = 30;

  const preferredLeft = btnRect.left + (btnRect.width / 2) - (menuWidth / 2);
  const minLeft = gapX;
  const maxLeft = window.innerWidth - menuWidth - gapX;
  const left = Math.min(Math.max(preferredLeft, minLeft), maxLeft);

  const spaceBelow = window.innerHeight - btnRect.bottom;
  const spaceAbove = btnRect.top;

  let top;
  if (spaceBelow < menuHeight + gapY && spaceAbove > menuHeight + gapY) {
    top = btnRect.top - menuHeight - gapY - 16;
  } else {
    top = btnRect.bottom + gapY;
  }

  menu.style.left = `${Math.round(left)}px`;
  menu.style.top = `${Math.round(top)}px`;

  menu.style.right = '';
  menu.style.zIndex = '2000';
  menu.style.width = `${menuWidth}px`;

  OPEN_DROPDOWN_ID = id;
}

function closeDropdown(id) {
  const menu = document.getElementById(`dropdown-menu-${id}`);
  if (menu) {
    menu.classList.add('hidden');
    menu.style.position = '';
    menu.style.top = '';
    menu.style.right = '';
    menu.style.width = '';
    menu.style.maxHeight = '';
    menu.style.zIndex = '';
  }
  if (OPEN_DROPDOWN_ID === id) OPEN_DROPDOWN_ID = null;
}

function closeAllDropdowns() {
  document.querySelectorAll('[id^="dropdown-menu-"]').forEach(m => {
    m.classList.add('hidden');
    m.style.position = '';
    m.style.top = '';
    m.style.right = '';
    m.style.width = '';
    m.style.maxHeight = '';
    m.style.zIndex = '';
  });
  OPEN_DROPDOWN_ID = null;
}

document.addEventListener('click', (e) => {
  if (!e.target.closest('[data-dropdown]')) closeAllDropdowns();
});

window.addEventListener('resize', () => {
  if (OPEN_DROPDOWN_ID) {
    const id = OPEN_DROPDOWN_ID;
    closeDropdown(id);
    openDropdown(id);
  }
});

window.addEventListener('scroll', () => {
  if (OPEN_DROPDOWN_ID) {
    const id = OPEN_DROPDOWN_ID;
    const menu = document.getElementById(`dropdown-menu-${id}`);
    const btn = document.querySelector(`[data-dropdown="${id}"] button`);
    if (menu && btn && !menu.classList.contains('hidden')) openDropdown(id);
  }
}, { passive: true });

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeAllDropdowns();
});
