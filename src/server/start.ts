import { app } from './index';

const port = Number(process.env.PORT || 4173);

console.log('Starting Specifi AI server...');
app.listen(port, '127.0.0.1', () => {
  console.log(`Specifi AI listening at http://127.0.0.1:${port}`);
});
