import express from 'express';

const app = express();
const port = process.env.PORT || 3000;

app.get('/', (_req, res) => {
  res.json({ message: 'Hello from {{name}}' });
});

app.listen(port, () => {
  console.log(`{{name}} listening on http://localhost:${port}`);
});
