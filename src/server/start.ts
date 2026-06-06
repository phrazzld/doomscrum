import { app } from './index';

const port = Number(process.env.PORT || 4173);

console.log('Starting PRD Brainrot Swipe server...');
app.listen(port, '127.0.0.1', () => {
  console.log(`PRD Brainrot Swipe listening at http://127.0.0.1:${port}`);
});
