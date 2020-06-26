<script>
  import Key from './Key.svelte';
  export let layout;

  let keylayout = [];
  let i = 0;
  for (let j of layout.layout) {
    let row = [];
    for (let k = 0; k < j; k++) {
      row.push(layout.keys[i]);
      i++;
    }
    keylayout.push(row);
  }
  console.log(keylayout);

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
    <Key legend={key.legend} size={key.size}/>
    {/each}
  </div>
  {/each}
</div>

