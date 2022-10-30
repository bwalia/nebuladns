<template>
    <div 
        class="hero" :class="{ 'hero--tall': tall }">
        <div class="hero-content">
            <slot></slot>
        </div>

        <div v-if="image" class="hero-image">
            <img :src="image" />
        </div>

        <div v-if="video" class="hero-image">
            <video autoplay="" muted="" loop="" :poster="(poster ? poster : '')">
                <source :src="video">
            </video>
        </div>

    </div>
</template>



<script>
export default {
  props: ['tall', 'image', 'video', 'poster'] // ~/assets/images/mountain.jpg
}
</script>



<style lang="scss" scoped>

@import "~/assets/css/variables.css";

.hero {
    border-radius: 30px;
    position: relative;
    // padding-top: 32%;
    min-height: 19em;
    background: #37495A;
    // background: rgb(55,73,90);
    // background: linear-gradient(126deg, rgba(55,73,90,1) 0%, rgba(170,205,221,0.5) 100%);
    width: 100%;
    overflow: hidden;
    margin-bottom: 2.4em;

    &.hero--tall {
        min-height: 24em;
    }

    &:after {
        position: absolute;
        top: 0;
        left: 0;
        content: ' ';
        background: linear-gradient(100deg, rgba(38,62,90,0.66), rgba(38,62,90,0));
        width: 100%; 
        height: 100%;
        z-index: 2;
    }

    .hero-image {
        position: absolute;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
        z-index: 1;
        img {
            opacity: 1;
            width: 100%;
            height: 100%;
            object-fit: cover;
            object-position: 60% 50%;
            @media (min-width: 400px) { object-position: 50% 50%; }
            @media (min-width: 800px) { object-position: 15% 50%; }
        }
        video {
            position: absolute;
            width: 100%;
            max-width: none;
            min-height: 100%;
            left: 0;
            top: 0;
            z-index: -1;
            object-fit: cover;
            object-position: 50% 50%;
            @media (min-width: 400px) { object-position: 30% 50%; }
            @media (min-width: 800px) { object-position: 40% 50%; }
        }
        
    }
    &.tile-focus-l img, 
    &.tile-focus-l video {
        width: 120%;
        object-position: 0% 50%;
        @media (min-width: 400px) { width: 140%;}
        @media (min-width: 800px) { width: 120%;}
    }

    .hero-content {
        position: relative;
        z-index: 3;
        width: calc(100% - 100px);
        color: #fff;
        padding: 30px;
        @media (min-width: 768px) { 
            width: 55%;
            padding: 30px 50px;
        }
    }

}

</style>