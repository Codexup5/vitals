#!/usr/bin/env node
import { appendFileSync } from 'node:fs';
import { createServer } from 'node:net';

const server = createServer();
server.listen(0, () => {
  const address = server.address();
  const port = typeof address === 'object' && address !== null ? address.port : null;
  server.close(() => {
    if (port === null) {
      console.error('ci-port: the server has no port');
      process.exit(1);
    }
    const line = `E2E_PORT=${port}`;
    console.log(line);
    if (process.env.GITHUB_ENV) appendFileSync(process.env.GITHUB_ENV, `${line}\n`);
  });
});
server.on('error', (error) => {
  console.error(`ci-port: ${error.message}`);
  process.exit(1);
});
