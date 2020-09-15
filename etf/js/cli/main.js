/**
 * Exercises the token-etf program
 *
 * @flow
 */

import {
  loadPrograms,
  createTokenEtf,
  deposit,
  withdraw,
} from './token-etf-test';

async function main() {
  // These test cases are designed to run sequentially and in the following order
  console.log('Run test: loadPrograms');
  await loadPrograms();
  console.log('Run test: createTokenEtf');
  await createTokenEtf();
  console.log('Run test: deposit');
  await deposit();
  console.log('Run test: withdraw');
  await withdraw();
  console.log('Success\n');
}

main()
  .catch(err => {
    console.error(err);
    process.exit(-1);
  })
  .then(() => process.exit());
