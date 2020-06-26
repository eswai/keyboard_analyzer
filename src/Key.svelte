<script>
  export let legend = [""];
  export let size = 1;
  export let value = 0;

  $: gb = 255 - value * 10;
  let width = 36 * size - 4;
</script>

<div class="key" style="width: {width}px; background-color:rgb(255, {gb}, {gb});">
  {#each legend as a}
  <div>{a}</div>
  {/each}
</div>

<style>

  .key {
    /* float: left; */
    margin: 1px;
    height:30px;
    background-color:#ffffff;
    border:1px solid #bebebe;
    border-radius:4px;
    /* display: grid; */
    /* grid-template-rows: 18px 18px; */
    /* grid-template-columns: 20px 20px; */
    font-size: 10px;
    /* align-items: baseline; */
  }

</style>