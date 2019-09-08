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
    alice: dna
  },
  bridges: [],
  debugLog: false,
  executor: tapeExecutor(require('tape')),
  middleware: backwardCompatibilityMiddleware,
})

diorama.registerScenario("Can register on network", async (s, t, { alice }) => {
  console.log("=============================================== REGISTER ON NETWORK");
  const createResult = await alice.call('my_zome', 'create_profile', { nickname: "alice" })
  console.log(createResult)
  t.equal(createResult.Err, undefined)
  t.notEqual(createResult.Ok, undefined)
})

diorama.registerScenario("Can get own profile", async (s, t, { alice }) => {
  console.log("=============================================== OWN PROFILE");
  const empty_result = await alice.call('my_zome', 'get_my_profile', { })
  console.log(empty_result)
  t.notEqual(empty_result.Err, undefined)
  t.equal(empty_result.Ok, undefined)

  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  let result = await alice.call('my_zome', 'get_my_profile', { })
  console.log(result)
  t.equal(result.Err, undefined)
  t.notEqual(result.Ok, undefined)
  t.notEqual(result.Ok.address, undefined)
  t.notEqual(result.Ok.entry, undefined)
  t.equal(result.Ok.entry.nickname, "alice")

  result = await alice.call('my_zome', 'get_profile', { profile_address: result.Ok.address })
  console.log(result)
  t.equal(result.Err, undefined)
  t.notEqual(result.Ok, undefined)
  t.notEqual(result.Ok.address, undefined)
  t.notEqual(result.Ok.entry, undefined)
  t.equal(result.Ok.entry.nickname, "alice")
})

diorama.registerScenario("Can create offer", async (s, t, { alice }) => {
  console.log("=============================================== CREATE OFFER");
  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  const createResult = await alice.call('my_zome', 'create_offer', { iam_offering: "apples", iam_requesting: [ "oranges", "goodwill" ] })
  console.log(createResult)
  t.equal(createResult.Err, undefined)
  t.notEqual(createResult.Ok, undefined)
})

diorama.registerScenario("Can remove offer", async (s, t, { alice }) => {
  console.log("=============================================== REMOVE OFFER");
  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  let createResult = await alice.call('my_zome', 'create_offer', { iam_offering: "apples", iam_requesting: [ "oranges", "goodwill" ] })

  const deleteResult = await alice.call('my_zome', 'remove_offer', { offer_address: createResult.Ok })
  console.log(deleteResult)
  t.equal(deleteResult.Err, undefined)
  t.notEqual(deleteResult.Ok, undefined)
})

diorama.registerScenario("Can find offer", async (s, t, { alice }) => {
  console.log("=============================================== FIND OFFER");
  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  await alice.call('my_zome', 'create_offer', { iam_offering: "apples", iam_requesting: [ "oranges", "goodwill" ] })

  const searchResult = await alice.call('my_zome', 'find_offers', { i_want: "" })
  console.log(JSON.stringify(searchResult, null, 2))
  t.equal(searchResult.Err, undefined)
  t.equal(searchResult.Ok.length, 1)

  const offer = searchResult.Ok[0]

  t.notEqual(offer.address, undefined)
  t.equal(offer.entry.offering, "apples")
  t.deepEqual(offer.entry.requesting, [ "oranges", "goodwill" ])
  t.notEqual(offer.entry.profile, undefined)
})

diorama.registerScenario("Can find a profile", async (s, t, { alice }) => {
  console.log("=============================================== FIND A PROFILE");
  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  const searchResult = await alice.call('my_zome', 'find_profiles', { nickname_prefix: "ali" })
  console.log(JSON.stringify(searchResult, null, 2))
  t.equal(searchResult.Err, undefined)
  t.equal(searchResult.Ok.length, 1)

  const profile = searchResult.Ok[0]

  t.notEqual(profile.address, undefined)
  t.equal(profile.entry.nickname, "alice")
  t.notEqual(profile.entry.address, undefined)
})

diorama.registerScenario("Can find deep offers when I WANT", async (s, t, { alice }) => {
  console.log("=============================================== FIND DEEP OFFERS I WANT");
  await alice.call('my_zome', 'create_profile', { nickname: "alice" })

  await alice.call('my_zome', 'create_offer', { iam_offering: "apples", iam_requesting: [ "oranges" ] })
  await alice.call('my_zome', 'create_offer', { iam_offering: "pears", iam_requesting: [ "oranges", "goodwill" ] })
  await alice.call('my_zome', 'create_offer', { iam_offering: "bananas", iam_requesting: [ "apples" ] })
  await alice.call('my_zome', 'create_offer', { iam_offering: "strawberries", iam_requesting: [ "apples" ] })
  await alice.call('my_zome', 'create_offer', { iam_offering: "strawberries", iam_requesting: [ "bananas" ] })

  // #1 oranges -> apples
  // #2 goodwill OR oranges -> pears
  // #3 apples -> bananas
  // #4 apples -> strawberries
  // #5 bananas -> strawberries


  // should return: #4, #5
  await try_query("strawberries", 1, 2)

  // should return: #4, #5, #1+#4, #3+#5
  await try_query("strawberries", 2, 4)

  // should return: #4, #5, #1+#4, #3+#5, #1+#3+#5
  await try_query("strawberries", 3, 5)

  async function try_query(i_want, max_swaps, expected_results) {
    const searchResult = await alice.call('my_zome', 'find_deep_offers_i_want', { i_want, max_swaps })
    console.log(JSON.stringify(searchResult, null, 2))
    t.equal(searchResult.Ok.length, expected_results)
  }
})


diorama.run()

