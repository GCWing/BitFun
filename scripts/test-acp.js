// Simple test client for BitFun ACP server
// Run with: node scripts/test-acp.js

const { spawn } = require('child_process');
const path = require('path');

// Check if bitfun-cli exists
const cliPath = path.join(__dirname, '..', 'target', 'debug', 'bitfun-cli');
const cliReleasePath = path.join(__dirname, '..', 'target', 'release', 'bitfun-cli');

const usePath = require('fs').existsSync(cliPath) ? cliPath : 
                require('fs').existsSync(cliReleasePath) ? cliReleasePath : 
                'bitfun-cli';

console.log('=== BitFun ACP Server Test (Node.js) ===\n');

// Test requests
const testRequests = [
  {
    name: 'Initialize',
    request: {
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: 1,
        clientCapabilities: {
          fs: { readTextFile: true, writeTextFile: true },
          terminal: true
        },
        clientInfo: { name: 'NodeTestClient', version: '1.0' }
      }
    }
  },
  {
    name: 'Create Session',
    request: {
      jsonrpc: '2.0',
      id: 2,
      method: 'session/new',
      params: {
        cwd: '/tmp/test-acp-node'
      }
    }
  },
  {
    name: 'List Tools',
    request: {
      jsonrpc: '2.0',
      id: 3,
      method: 'tools/list'
    }
  }
];

// Run individual tests
async function runTest(test) {
  console.log(`Test: ${test.name}`);
  console.log('Request:', JSON.stringify(test.request, null, 2));
  
  const child = spawn(usePath, ['acp'], {
    stdio: ['pipe', 'pipe', 'inherit']
  });
  
  let output = '';
  
  child.stdout.on('data', (data) => {
    output += data.toString();
  });
  
  child.stdin.write(JSON.stringify(test.request) + '\n');
  child.stdin.end();
  
  return new Promise((resolve) => {
    child.on('close', (code) => {
      console.log('Response:', output);
      try {
        const response = JSON.parse(output);
        console.log('Parsed:', JSON.stringify(response, null, 2));
      } catch (e) {
        console.log('Parse error:', e.message);
      }
      console.log('\n');
      resolve();
    });
  });
}

// Run all tests sequentially
async function runAllTests() {
  for (const test of testRequests) {
    await runTest(test);
  }
  
  console.log('=== Tests Complete ===');
}

runAllTests().catch(console.error);