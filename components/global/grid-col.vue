<template>
    <div :class="`col${cols}`">
        <slot></slot>
    </div>
</template>

<script>
export default {
  props: ['cols']
}
</script>

<style lang="scss" scoped>

@import "~/assets/css/variables.css";

.col1, .col2, .col3, .col4, .col5, .col6, .col7, .col8, .col9, .col10, .col11, .col12,
col2up, col3up, col4up {
    margin: 0 4%;
    flex: 0 0 100%;
}

/* 100 - (4 * (cols - 1)) / cols */
@media (min-width: 768px) {
    .col2up { flex-basis: 48%; } 
    .col3up { flex-basis: 30.667%; }
    .col4up { flex-basis: 22%; }
    .col1 { flex: 0 0 8.333%; } 
    .col2 { flex: 0 0 16.666%; }
    .col3 { flex: 0 0 25%; }
    .col4 { flex: 0 0 33.333%; } 
    .col5 { flex: 0 0 41.666%; }
    .col6 { flex: 0 0 50%; }
    .col7 { flex: 0 0 58.333%; }
    .col8 { flex: 0 0 66.666%; }
    .col9 { flex: 0 0 75%; }
    .col10 { flex: 0 0 83.333%; }
    .col11 { flex: 0 0 91.666%; }
    .col12 { flex: 0 0 100%; }

    /* .col1, .col2, .col3, .col4, .col5, .col6, .col7, .col8, .col9, .col10, .col11, .col12 {
        padding-right: 20px;
    } */
}


</style>