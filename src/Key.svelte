<script>
  export let legend = [""];
  export let size = 1;
  export let value = 0;
  export let count = 0;
  export let color = "mono";

  let width = 36 * size - 4;

  let csskey;
  let csscnt;
  switch (legend.length) {
    case 0:
      csskey = "keynone";
      csscnt = "cnt1led";
      break;
    case 1:
      csskey = "key1led";
      csscnt = "cnt1led";
      break;
    case 2:
      csskey = "key2led";
      csscnt = "cnt2led";
      break;
    default:
      csskey = "key3led";
      csscnt = "cnt3led";
      break;
  }

  function heatmap(v){
    const scale = 20;
    let l, h;
    switch (color) {
    case "mono":
      if (v > 0) {
        l = 65 + (1 - Math.log2(v * scale + 1) / Math.log2(scale + 1)) * 35;
        return "hsl(0, 100%, " + l + "%)";
            } else {
        return "white";
      }
    case "multi":
      if (v > 0) {
        // let h = (1.0 - Math.log2(v * scale + 1) / Math.log2(scale + 1)) * 100;
        h = (1.0 - v) * 100;
        return "hsl(" + h + ", 85%, 60%)";
      } else {
        return "white";
      }
    case "random":
      if (v > 0) {
        l = Math.floor(v * 10 % 10);
        return ["#ffadd6", "#d6adff", "#d6ffad", "#adffd6", "#add6ff", "#adffff", "#adadff", "#ffadff", "#adffad", "#ffffad"][l];
      } else {
        return "white";
      }
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
  .keynone {
    /* float: left; */
    margin: 1px;
    height:30px;
    background-color:#ffffff;
    border: 1px solid white;
    border-radius:4px;
    display: grid;
    grid-template-rows: 15px 15px;
    font-size: 10px;
    /* align-items: baseline; */
  }

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

   .key3led {
    /* float: left; */
    margin: 1px;
    height:30px;
    background-color:#ffffff;
    border:1px solid #bebebe;
    border-radius:4px;
    display: grid;
    grid-template-rows: 15px 15px;
    grid-template-columns: 10px 10px 10px;
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

  .cnt3led {
    color: #79402f;
    grid-column: 1 / 4;
  }
</style>