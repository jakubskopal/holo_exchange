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

diorama.registerScenario("Can get own profile", async (s, t, { alice }) => {
  console.log("=============================================== OWN PROFILE");
  const empty_result = await alice.call('my_zome', 'get_my_profile', { })
  console.log(empty_result)
  t.notEqual(empty_result.Err, undefined)
  t.equal(empty_result.Ok, undefined)

  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  const result = await alice.call('my_zome', 'get_my_profile', { })
  console.log(result)
  t.equal(result.Err, undefined)
  t.notEqual(result.Ok, undefined)
  t.notEqual(result.Ok.address, undefined)
  t.notEqual(result.Ok.entry, undefined)
  t.equal(result.Ok.entry.nickname, "alice")
})

// diorama.registerScenario("Can create exchange", async (s, t, { alice }) => {
//   console.log("=============================================== CREATE EXCHANGE");
//   await alice.call('my_zome', 'create_profile', { nickname: "alice" })
//
//   const createResult = await alice.call('my_zome', 'create_exchange', { offering: "apples", requesting: "oranges" })
//   console.log(createResult)
//   t.equal(createResult.Err, undefined)
//   t.notEqual(createResult.Ok, undefined)
// })

// diorama.registerScenario("Can find exchange", async (s, t, { alice, bob }) => {
//   console.log("=============================================== FIND EXCHANGE");
//   await alice.call('my_zome', 'create_profile', { nickname: "alice" })
//   await bob.call('my_zome', 'create_profile', { nickname: "bob" })
//
//   await alice.call('my_zome', 'create_exchange', { offering: "apples", requesting: "oranges" })
//
//   const searchResult = await bob.call('my_zome', 'find_exchanges', { offering: "", requesting: "" })
//   console.log(JSON.stringify(searchResult, null, 2))
//   t.equal(searchResult.Err, undefined)
//   t.equal(searchResult.Ok.length, 1)
//
//   const exchange = searchResult.Ok[0]
//
//   t.notEqual(exchange.address, undefined)
//   t.equal(exchange.entry.offering, "apples")
//   t.equal(exchange.entry.requesting, "oranges")
//   t.notEqual(exchange.entry.profile, undefined)
// })

// diorama.registerScenario("Can find a profile", async (s, t, { alice, bob }) => {
//   console.log("=============================================== FIND A PROFILE");
//   await alice.call('my_zome', 'create_profile', { nickname: "alice" })
//
//   const searchResult = await bob.call('my_zome', 'find_profiles', { nickname_prefix: "ali" })
//   console.log(JSON.stringify(searchResult, null, 2))
//   t.equal(searchResult.Err, undefined)
//   t.equal(searchResult.Ok.length, 1)
//
//   const profile = searchResult.Ok[0]
//
//   t.notEqual(profile.address, undefined)
//   t.equal(profile.entry.nickname, "alice")
//   t.notEqual(profile.entry.address, undefined)
// })

diorama.run()

