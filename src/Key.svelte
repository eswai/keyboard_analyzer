<script>
  export let legend = [""];
  export let size = 1;
  export let value = 0;
  export let count = 0;

  let width = 36 * size - 4;

  let csskey = legend.length > 1 ? "key2led" : "key1led";
  let csscnt = legend.length > 1 ? "cnt2led" : "cnt1led";

  function heatmap(v){
    const scale = 20;
    if (v > 0) {
      // let h = (1.0 - Math.log2(v * scale + 1) / Math.log2(scale + 1)) * 100;
      // let h = (1.0 - v) * 100;
      // return "hsl(" + h + ", 85%, 60%)";
      let l = 65 + (1 - Math.log2(v * scale + 1) / Math.log2(scale + 1)) * 35;
      return "hsl(0, 100%, " + l + "%)";
    } else {
      return "white";
    }
    
  }
</script>

<div class={csskey} style="width: {width}px; background-color:{heatmap(value)};">
  {#each legend as a}
  <div>{a}</div>
  {/each}
  {#if count > 0}
  <div class={csscnt}>{count}</div>
  {/if}
</div>

<style>

  .key1led {
    /* float: left; */
    margin: 1px;
    height:30px;
    background-color:#ffffff;
    border:1px solid #bebebe;
    border-radius:4px;
    display: grid;
    grid-template-rows: 15px 15px;
    font-size: 10px;
    /* align-items: baseline; */
  }
   .key2led {
    /* float: left; */
    margin: 1px;
    height:30px;
    background-color:#ffffff;
    border:1px solid #bebebe;
    border-radius:4px;
    display: grid;
    grid-template-rows: 15px 15px;
    grid-template-columns: 15px 15px;
    font-size: 10px;
    /* align-items: baseline; */
  }

  .cnt1led {
    color: #79402f;
  }

  .cnt2led {
    color: #79402f;
    grid-column: 1 / 3;
  }

</style>