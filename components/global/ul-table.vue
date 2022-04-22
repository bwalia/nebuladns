<template>
    <ul class="ul-table">
        <slot></slot>
    </ul>
</template>

<script>
export default {
  props: []
}
</script>

<style lang="scss" scoped>

@import "~/assets/css/variables.css";

.ul-table {
padding: 0;
}
.ul-table li {
    line-height: 42px;
    list-style: none;
    margin: 0;
    padding: 0;
    border-bottom: 1px solid var(--default);
}


</style>