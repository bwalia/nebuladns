<template>
    <div class="report-item" :class="{ 'is-open': isOpen }">
        <h2 @click="toggle()">
            <span>{{subhead}}</span> 
            <svg><use xlink:href="#twisty"></use></svg>
        </h2>
        <div class="report-content">

            <template v-if="!ischart">
                <div class="report-layout" v-html="content" />
            </template>

            <template v-if="ischart">
                
                <!-- <pre>{{chart.data}}</pre> -->
                <!-- <pre>{{chart.options}}</pre> -->
                <!-- <pre>{{content}}</pre> -->
                <!-- <pre>{{chartObject}}</pre> -->
                <!-- <div class="report-layout" v-html="content" /> -->
                <template v-if="chartObject">
                    <line-chart class="line-chart" :data="chart.data" :options="chart.options" />
                </template>
            </template>

        </div>
    </div>
</template>

<script>
export default {
    props: ['subhead', 'content', 'ischart', 'documents'],
    data () {
        return {
            isOpen: true,
            temp: {
                cleaned: '',
                lines: []
            },
            chartObject: {},
            chart: {
                data: {
                    labels: ["Q3'20","Q4'20","Q1'21","Q2'21","Q3'21","Q4'21","Q1'22"],
                    datasets: [
                        {
                            label: "% increase since inception",
                            borderColor: "#1790E3",
                            borderWidth: 5,
                            fill: false,
                            data: [0, 1.6, 2.4, 2.5, 4.1, 6.2, 7, 8.8]
                        },
                        {
                            label: "Benchmark",
                            borderColor: "#d5d5d5",
                            borderWidth: 5,
                            fill: false,
                            data: [0, 1, 2, 3, 4, 5, 6, 7]
                        }
                    ]
                },
                options: {
                    maintainAspectRatio: false,
                    responsive: true
                }
            }
        }
    },
    methods: {
        toggle: function(){
            this.isOpen = !this.isOpen
        }
        // stringToKeyPair: function(string) {
        //     var keyPair = {};
        //     var split = string.split(':');
        //     keyPair[split[0]] = split[1];
        //     return keyPair;
        // }
    },
    mounted: function() {
        if (this.ischart) {
            this.temp.cleaned = this.content.replaceAll('\"','');
            this.temp.lines = this.temp.cleaned.split('\r\n');    
            this.temp.lines.map(line => {
                var pair = line.split(':');
                this.chartObject[pair[0]] = pair[1]; 
            }); 
            console.log(this.chartObject);
            // this.chart.data.datasets[0].data = this.chartObject.chartData;    
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