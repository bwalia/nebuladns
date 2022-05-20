<template>
    <div class="report-item" :class="{ 'is-open': isOpen }">
        <h2 @click="toggle()">
            <span>{{subhead}}</span> 
            <svg><use xlink:href="#twisty"></use></svg>
        </h2>
        <div class="report-content">

            <template v-if="!chart">
                <div class="report-layout" v-html="content" />
            </template>

            <template v-if="chart">
                
                <!-- <pre>{{chart.data}}</pre> -->
                <!-- <pre>{{chart.options}}</pre> -->
                <pre>{{chartObject}}</pre>
                <!-- <div class="report-layout" v-html="content" /> -->
                <!-- <line-chart class="line-chart" :data="chart.data" :options="chart.options" /> -->
            </template>

        </div>
    </div>
</template>

<script>
export default {
    props: ['subhead', 'content', 'chart', 'documents'],
    data () {
        return {
            isOpen: true,
            chartObject: {}
        }
    },
    methods: {
        toggle: function(){
            this.isOpen = !this.isOpen
        }
    },
    mounted: function() {
        if (this.chart) {
            this.chartObject = this.content;
        } 
    }
}
</script>

<style lang="scss">

.report-item {
    border: 1px solid var(--rule);
    border-width: 0px 1px 1px;
}

.report-item h2 {
    font-size: var(--font2);
    margin: 0;
    padding: 0 0.75em;
    line-height: 56px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: nowrap;
    border-bottom: 1px solid #fff;
    border-collapse: collapse;

}

.report-item .report-content {
    background: var(--tint);
    max-height: 0;
    transition: 0.2s max-height ease;
    overflow: hidden;
}

.report-item h2 svg {
    width: 16px;
    height: 16px;
    transform: rotate(90deg);
    transition: 0.2s transform ease;
    background-color: var(--tint);
    border-radius: 50%;
    box-shadow: 0 0 0 4px var(--tint);
    margin-right: 0.5em;
}

.report-item.is-open .report-content {
    max-height: 999px; 
}

.report-item.is-open h2 {
    max-height: none; 
    border-bottom: 1px solid var(--rule);
}
.report-item.is-open h2 svg {
    transform: rotate(180deg);
}

.report-layout {
    margin: 2em;
    @media (min-width: 640px) {
        column-count: 2;
        column-gap: 4%;
    }
}

.report-layout *:first-child {
    margin-top: 0;
}

.line-chart {
    width: auto;
    margin: 30px;
}

</style>