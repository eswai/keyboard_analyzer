<script>
  export let press = false;
  export let legend = [""];
  export let size = 1;

  $: css = press ? "keyon" : "keyoff"
</script>

<div class="key {css}" style="width: {36 * size - 4}px">
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
  
  .keyon {
    background-color:#caffbd;
  }

  .keyoff {
    background-color:#ffffff;
  }

</style>