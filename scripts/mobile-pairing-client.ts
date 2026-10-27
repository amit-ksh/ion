import QRCode from 'qrcode';
import { parseDaemonPairingCode } from '../apps/mobile/src/lib/daemon-pairing';

const form = document.querySelector<HTMLFormElement>('form')!;
const canvas = document.querySelector<HTMLCanvasElement>('canvas')!;
const result = document.querySelector<HTMLElement>('#result')!;
const error = document.querySelector<HTMLElement>('#error')!;
function clear() {
  result.hidden = true;
  canvas.getContext('2d')?.clearRect(0, 0, canvas.width, canvas.height);
}
form.addEventListener('input', clear);
form.addEventListener('submit', async (event) => {
  event.preventDefault();
  clear();
  error.textContent = '';
  try {
    const fields = new FormData(form);
    const data = JSON.stringify({
      type: 'ion-pairing',
      version: 1,
      name: fields.get('name'),
      address: fields.get('address'),
      token: fields.get('token'),
    });
    parseDaemonPairingCode(data);
    await QRCode.toCanvas(canvas, data, {
      width: 360,
      margin: 4,
      errorCorrectionLevel: 'M',
    });
    result.hidden = false;
  } catch {
    error.textContent =
      'Enter a ws:// address on your private network (or wss:// address), a token, and an optional name.';
  }
});
document.querySelector('#clear')!.addEventListener('click', () => {
  form.reset();
  clear();
  error.textContent = '';
});
window.addEventListener('pagehide', () => {
  form.reset();
  clear();
});
