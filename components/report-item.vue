<template>
    <div class="report-item" :class="{ 'is-open': isOpen }">
        <h2 @click="toggle()">
            <span>{{(subhead ? subhead : ' ')}}</span> 
            <svg><use xlink:href="#twisty"></use></svg>
        </h2>
        <div class="report-content">

            <!-- <pre>{{content}}</pre> -->
            <!-- <pre>{{chartDataObject}}</pre> -->

            <template v-if="!ischart">
                <div class="report-layout" v-html="content" />
            </template>
            
            <template v-if="ischart">
                <VueSlickCarousel :arrows="true" :dots="true">
                    <div v-for="(chart, index) in this.chartDataObject.chartData">
                    <line-chart
                        class="line-chart" 
                        :series="(chartDataObject && chartDataObject.chartData[index])
                            ? chartDataObject.chartData[index]
                            : []" 
                        :benchmark="(chartDataObject && chartDataObject.benchmarkData[index])
                            ? chartDataObject.benchmarkData[index]
                            : []"
                        :labels="(chartDataObject && chartDataObject.chartLabels[index])
                            ? chartDataObject.chartLabels[index]
                            : []" />
                    </div>
                </VueSlickCarousel>
            </template>

        </div>
    </div>
</template>

<script>
  import VueSlickCarousel from 'vue-slick-carousel'
  import 'vue-slick-carousel/dist/vue-slick-carousel.css'
  import 'vue-slick-carousel/dist/vue-slick-carousel-theme.css'
export default {
    props: ['subhead', 'content', 'ischart', 'documents'],
    data () {
        return {
            isOpen: true,
            temp: {
                cleaned: '',
                lines: []
            }
        }
    },
    components: { VueSlickCarousel },
    methods: {
        toggle: function(){
            this.isOpen = !this.isOpen
        }
    },
    computed: {
        splitChartData: function() {
            if (this.ischart && this.content) {
                return this.content.split(/\r?\n/);
            } else {
                return false;
            }
        },
        chartDataObject: function() {
            let dataObject = {};
            var size = 61;
            if(this.splitChartData) {
                this.splitChartData.forEach((item, index) => {
                    let splitRecord = item.split(':');
                    if(splitRecord && splitRecord.length > 1) {
                        let dataArray = splitRecord[1].split(',');
                        const res = dataArray.reduce((acc, curr, i) => {
                        if ( !(i % size)  ) { 
                            acc.push(dataArray.slice(i, i + size)); 
                        }
                        return acc;
                        }, []);
                        dataObject[splitRecord[0]] = res;
                    }
                });
                return dataObject;
            }
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

.slick-arrow.slick-prev {
    left: 0px
}

.slick-arrow.slick-next {
    right: 0;
}

.slick-prev:before, .slick-next:before {
    color: #397BAA;
}

</style>