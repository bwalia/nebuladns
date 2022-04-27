<template>
    <div class="report-list">

        <svg style="display: none;">
            <defs>
                <symbol id="twisty" viewBox="0 0 16 16">
                    <title>Twisty</title>
                    <polyline fill="none" stroke="#010202" stroke-width="2" stroke-miterlimit="10" points="2,11 8,5 14,11 "/>
                </symbol>
            </defs>
        </svg>
        
        <report-item 
            v-for="(item, i) in reportdata"
            :key="i" 
            :subhead="(item.title) ? item.title : 'Section'"
            :content="(item.text) ? item.text : null"
            :chart="null" />
    </div>
</template>

<script>
export default {
    props: ['reportdata'],
}
</script>

<style lang="css" scoped>
.report-list {
    border-top: 2px solid var(--default);
    margin-top: 2em;
}


</style>