<script>
  import Key from './Key.svelte';
  export let layout;
  let keylayout = [];

  $: update(layout);

  function update(l) {
    keylayout = [];
    let i = 0;
    for (let j of l.layout) {
      let row = [];
      for (let k = 0; k < j; k++) {
        row.push(l.keys[i]);
        i++;
      }
      keylayout.push(row);
    }
    // console.log(keylayout)
  }
</script>

<style>

  .keyboard {
    margin: auto;
    text-align: center;
  }

  .row {
    display: flex;
  }

</style>

<div class="keyboard">
  {#each keylayout as row}
  <div class="row">
    {#each row as key}
    <Key legend={key.legend} size={key.size} value={key.value} count={key.count}/>
    {/each}
  </div>
  {/each}
</div>

