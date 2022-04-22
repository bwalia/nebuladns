<template>

  <div class="part" :class="{
    'part--flush' : flush,
    'part--reset' : reset,
    'u-center' : center
  }">

    <div class="part-content">
      <slot></slot>
    </div>

  </div>

</template>

<script>
export default {
  props: [
    'center', // boolean, default false
    'flush', // boolean, vertical spacing, default false
    'reset' // boolean, horizontal butt to edges at all sizes, eg sliders
  ], // 
  data () {
    return {
     
    }
  }
}
</script>

<style lang="css" scoped>

@import "~/assets/css/variables.css";

.part {
    padding: 1em 2%;
    width: auto;
    display: flex;
    justify-content: center;
}
  
.part .part-content {
    width: 100%;
}

@media (min-width: 800px) {
    .part .part-content {
        max-width: calc(29vw + 640px);
    }
}

.part.part--flush {
    padding-top: 0;
    padding-bottom: 0;
}

.part.part--reset {
    padding-left: 0;
    padding-right: 0;
}

@media (min-width: 800px) {
    .part.part--reset .part-content {
        max-width: none;
    }
}

</style>
