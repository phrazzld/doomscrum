import path from 'node:path';

export const projectRoot = process.cwd();
export const backlogDir = path.join(projectRoot, 'backlog.d');
export const brainrotDir = path.join(projectRoot, '.brainrot');
export const storyboardsDir = path.join(brainrotDir, 'storyboards');
export const rendersDir = path.join(brainrotDir, 'renders');
export const runPacketsDir = path.join(brainrotDir, 'run-packets');
export const eventsPath = path.join(brainrotDir, 'events.ndjson');
export const indexPath = path.join(brainrotDir, 'index.json');
export const distDir = path.join(projectRoot, 'dist');
