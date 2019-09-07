const path = require('path')
const tape = require('tape')

const { Diorama, tapeExecutor, backwardCompatibilityMiddleware } = require('@holochain/diorama')

process.on('unhandledRejection', error => {
  // Will print "unhandledRejection err is not defined"
  console.error('got unhandledRejection:', error);
});

const dnaPath = path.join(__dirname, "../dist/my_first_app.dna.json")
const dna = Diorama.dna(dnaPath, 'my_first_app')

const diorama = new Diorama({
  instances: {
    alice: dna,
    bob: dna,
  },
  bridges: [],
  debugLog: false,
  executor: tapeExecutor(require('tape')),
  middleware: backwardCompatibilityMiddleware,
})

// diorama.registerScenario("Can register on network", async (s, t, { alice }) => {
//   console.log("=============================================== REGISTER ON NETWORK");
//   const createResult = await alice.call('my_zome', 'create_profile', { nickname: "alice" })
//   console.log(createResult)
//   t.equal(createResult.Err, undefined)
//   t.notEqual(createResult.Ok, undefined)
// })

// diorama.registerScenario("Can create exchange", async (s, t, { alice }) => {
//   console.log("=============================================== CREATE EXCHANGE");
//   await alice.call('my_zome', 'create_profile', { nickname: "alice" })
//
//   const createResult = await alice.call('my_zome', 'create_exchange', { offering: "apples", requesting: "oranges" })
//   console.log(createResult)
//   t.equal(createResult.Err, undefined)
//   t.notEqual(createResult.Ok, undefined)
// })

diorama.registerScenario("Can find exchange", async (s, t, { alice, bob }) => {
  console.log("=============================================== FIND EXCHANGE");
  await alice.call('my_zome', 'create_profile', { nickname: "alice" })
  await bob.call('my_zome', 'create_profile', { nickname: "bob" })

  await alice.call('my_zome', 'create_exchange', { offering: "apples", requesting: "oranges" })

  const searchResult = await bob.call('my_zome', 'find_exchanges', { offering: "", requesting: "" })
  console.log(JSON.stringify(searchResult, null, 2))
  t.equal(searchResult.Err, undefined)
  t.equal(searchResult.Ok.length, 1)

  const exchange = searchResult.Ok[0]

  t.notEqual(exchange.address, undefined)
  t.equal(exchange.entry.offering, "apples")
  t.equal(exchange.entry.requesting, "oranges")
  t.notEqual(exchange.entry.profile, undefined)
})

diorama.run()

// diorama.registerScenario('Can add some items', async (s, t, { alice }) => {
//   const createResult = await alice.call('my_zome', 'create_list', { list: { name: 'test list' } })
//   const listAddr = createResult.Ok
//
//   const result1 = await alice.call('my_zome', 'add_item', { list_item: { text: 'Learn Rust', completed: true }, list_addr: listAddr })
//   const result2 = await alice.call('my_zome', 'add_item', { list_item: { text: 'Master Holochain', completed: false }, list_addr: listAddr })
//
//   console.log(result1)
//   console.log(result2)
//
//   t.notEqual(result1.Ok, undefined)
//   t.notEqual(result2.Ok, undefined)
// })
//
// diorama.registerScenario('Can get a list with items', async (s, t, { alice }) => {
//   const createResult = await alice.call('my_zome', 'create_list', { list: { name: 'test list' } })
//   const listAddr = createResult.Ok
//
//   await alice.call('my_zome', 'add_item', { list_item: { text: 'Learn Rust', completed: true }, list_addr: listAddr })
//   await alice.call('my_zome', 'add_item', { list_item: { text: 'Master Holochain', completed: false }, list_addr: listAddr })
//
//   const getResult = await alice.call('my_zome', 'get_list', { list_addr: listAddr })
//   console.log(getResult)
//
//   t.equal(getResult.Ok.items.length, 2, 'there should be 2 items in the list')
// })


